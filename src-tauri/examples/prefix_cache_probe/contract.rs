use std::{ffi::OsString, path::PathBuf};

pub struct Options {
    pub model: PathBuf,
    pub manifest: PathBuf,
    pub output: PathBuf,
}
impl Options {
    pub fn parse(args: &[OsString]) -> Result<Self, &'static str> {
        if args.len() != 3
            || args
                .iter()
                .any(|arg| !(1..=4096).contains(&arg.as_encoded_bytes().len()))
        {
            return Err(
                "usage: prefix_cache_probe MODEL MANIFEST NEW_OUTPUT; three bounded explicit paths required",
            );
        }
        Ok(Self {
            model: args[0].clone().into(),
            manifest: args[1].clone().into(),
            output: args[2].clone().into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exactly_three_explicit_bounded_paths_are_required() -> Result<(), &'static str> {
        let args = ["model", "manifest.json", "new.json"].map(OsString::from);
        let options = Options::parse(&args)?;
        assert_eq!(options.model, PathBuf::from("model"));
        assert_eq!(options.manifest, PathBuf::from("manifest.json"));
        assert_eq!(options.output, PathBuf::from("new.json"));
        for size in 0..3 {
            assert!(Options::parse(&args[..size]).is_err());
        }
        let mut extra = args.to_vec();
        extra.push("--fake-hit".into());
        assert!(Options::parse(&extra).is_err());
        for value in [OsString::new(), OsString::from("x".repeat(4097))] {
            for i in 0..3 {
                let mut invalid = args.clone();
                invalid[i] = value.clone();
                assert!(Options::parse(&invalid).is_err());
            }
        }
        Ok(())
    }
}

use everlast_dictation::transcription::prefix_cache::{PreparedReuse, ReuseOutcome};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Prefix,
    SameSessionWhole,
    NewSessionWhole,
}

pub struct Expected {
    pub session: u64,
    pub generation: u64,
    pub job_id: u64,
    pub role: Role,
}

pub fn validate_pair(
    prefix_language: &str,
    whole_language: &str,
    prefix: &[f32],
    whole: &[f32],
) -> Result<(), &'static str> {
    if prefix_language != whole_language
        || !matches!(prefix_language, "de" | "en")
        || prefix.is_empty()
        || prefix.len() > whole.len()
        || whole.len() > 4_800_000
        || !prefix
            .iter()
            .zip(whole)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    {
        return Err(
            "prefix must be a nonempty bit-identical initial range of the same-language whole input",
        );
    }
    Ok(())
}

pub fn validate_receipt(
    expected: &Expected,
    receipt: &PreparedReuse,
    origin: Option<&PreparedReuse>,
) -> Result<(), &'static str> {
    if receipt.session != expected.session
        || receipt.generation != expected.generation
        || receipt.job_id != expected.job_id
        || expected.session == 0
        || expected.generation == 0
        || expected.job_id == 0
    {
        return Err("actual decoder receipt identity differs");
    }
    if expected.role == Role::NewSessionWhole
        && (receipt.outcome != ReuseOutcome::Decoded
            || !receipt.decoder_invoked
            || !receipt.native_full_attempted)
    {
        return Err("fresh session whole must perform a real native decode");
    }
    match receipt.outcome {
        ReuseOutcome::Decoded => {
            if !receipt.decoder_invoked
                || receipt.origin_job_id != Some(expected.job_id)
                || !matches!(receipt.prepared_frames, Some(1..=4_800_000))
                || receipt.prepared_pcm_sha256.is_none()
            {
                return Err("decoded receipt lacks current-origin prepared evidence");
            }
        }
        ReuseOutcome::Hit => {
            let origin = origin.ok_or("cache hit lacks prefix receipt")?;
            if expected.role != Role::SameSessionWhole
                || origin.outcome != ReuseOutcome::Decoded
                || origin.session != expected.session
                || origin.generation != expected.generation
                || receipt.origin_job_id != Some(origin.job_id)
                || origin.job_id >= expected.job_id
                || receipt.decoder_invoked
                || receipt.native_full_attempted
                || !matches!(receipt.prepared_frames, Some(1..=4_800_000))
                || receipt.prepared_frames != origin.prepared_frames
                || receipt.prepared_pcm_sha256.is_none()
                || receipt.prepared_pcm_sha256 != origin.prepared_pcm_sha256
            {
                return Err("hit differs from the actual prefix prepared input/origin");
            }
        }
        ReuseOutcome::NotAdmitted => {
            if receipt.decoder_invoked
                || receipt.native_full_attempted
                || receipt.origin_job_id.is_some()
                || receipt.prepared_frames.is_some()
                || receipt.prepared_pcm_sha256.is_some()
            {
                return Err("non-admitted receipt claims native or cache work");
            }
        }
        ReuseOutcome::Failed | ReuseOutcome::InvalidContext | ReuseOutcome::Revoked => {
            return Err("actual cache decoder failed or revoked its context");
        }
    }
    Ok(())
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    fn decoded(job: u64) -> PreparedReuse {
        PreparedReuse {
            session: 1,
            generation: 1,
            job_id: job,
            outcome: ReuseOutcome::Decoded,
            origin_job_id: Some(job),
            prepared_frames: Some(16000),
            prepared_pcm_sha256: Some([7; 32]),
            decoder_invoked: true,
            native_full_attempted: true,
        }
    }
    #[test]
    fn prefix_must_match_whole_pcm_bits_without_normalization() -> Result<(), &'static str> {
        validate_pair("de", "de", &[0.25, -0.5], &[0.25, -0.5, 0.75])?;
        for (lang, prefix, whole) in [
            ("en", vec![0.25], vec![0.25]),
            ("de", vec![], vec![0.25]),
            ("de", vec![0.25, 0.5], vec![0.25]),
            ("de", vec![0.0], vec![-0.0]),
            ("de", vec![0.25], vec![0.5]),
        ] {
            assert!(validate_pair("de", lang, &prefix, &whole).is_err());
        }
        Ok(())
    }
    #[test]
    fn hit_binds_prefix_origin_and_fresh_session_must_decode() -> Result<(), &'static str> {
        let origin = decoded(1);
        let expected = Expected {
            session: 1,
            generation: 1,
            job_id: 2,
            role: Role::SameSessionWhole,
        };
        let mut hit = origin.clone();
        hit.job_id = 2;
        hit.outcome = ReuseOutcome::Hit;
        hit.decoder_invoked = false;
        hit.native_full_attempted = false;
        validate_receipt(&expected, &hit, Some(&origin))?;
        let mut fresh = decoded(3);
        fresh.session = 2;
        let new = Expected {
            session: 2,
            generation: 1,
            job_id: 3,
            role: Role::NewSessionWhole,
        };
        validate_receipt(&new, &fresh, Some(&origin))?;
        hit.session = 2;
        hit.job_id = 3;
        assert!(validate_receipt(&new, &hit, Some(&origin)).is_err());
        for change in 0..6 {
            let mut wrong = fresh.clone();
            match change {
                0 => wrong.session = 1,
                1 => wrong.generation = 2,
                2 => wrong.origin_job_id = Some(1),
                3 => wrong.prepared_pcm_sha256 = None,
                4 => wrong.native_full_attempted = false,
                _ => wrong.decoder_invoked = false,
            }
            assert!(validate_receipt(&new, &wrong, None).is_err());
        }
        Ok(())
    }
    #[test]
    fn hits_cannot_claim_native_decode_or_different_prepared_pcm() {
        let origin = decoded(1);
        let expected = Expected {
            session: 1,
            generation: 1,
            job_id: 2,
            role: Role::SameSessionWhole,
        };
        for change in 0..5 {
            let mut hit = origin.clone();
            hit.job_id = 2;
            hit.outcome = ReuseOutcome::Hit;
            hit.decoder_invoked = false;
            hit.native_full_attempted = false;
            match change {
                0 => hit.prepared_pcm_sha256 = Some([8; 32]),
                1 => hit.prepared_frames = Some(8000),
                2 => hit.origin_job_id = Some(7),
                3 => hit.decoder_invoked = true,
                _ => hit.native_full_attempted = true,
            }
            assert!(validate_receipt(&expected, &hit, Some(&origin)).is_err());
        }
    }
}
