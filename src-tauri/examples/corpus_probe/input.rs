use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Cursor, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Component, Path},
};

pub const MAX_MANIFEST: u64 = 16 * 1024 * 1024;
pub const MAX_AUDIO: u64 = 9_665_536;
const MAX_FRAMES: u32 = 4_800_000;
type Error = Box<dyn std::error::Error>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub cases: Vec<Case>,
}
#[derive(Deserialize)]
pub struct Case {
    pub id: String,
    pub language: String,
    pub audio: Audio,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audio {
    pub path: String,
    pub sha256: String,
    pub frames: u32,
    pub sample_rate: u32,
}

pub fn bounded_read(path: &Path, maximum: u64) -> Result<Vec<u8>, Error> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err("input is not a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("input exceeded its size limit".into());
    }
    Ok(bytes)
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, Error> {
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("manifest exceeds 16 MiB".into());
    }
    let manifest: Manifest = serde_json::from_slice(bytes)?;
    if manifest.schema_version != 1 || manifest.cases.is_empty() || manifest.cases.len() > 500 {
        return Err("invalid manifest version or case count".into());
    }
    let mut ids = HashSet::new();
    for case in &manifest.cases {
        if case.id.is_empty()
            || case.id.len() > 128
            || !case.id.bytes().all(|b| b.is_ascii_graphic())
            || !ids.insert(&case.id)
        {
            return Err("invalid or duplicate case identifier".into());
        }
        if !matches!(case.language.as_str(), "de" | "en") {
            return Err("unsupported case language".into());
        }
        let audio = &case.audio;
        if audio.frames == 0
            || audio.frames > MAX_FRAMES
            || audio.sample_rate != 16000
            || audio.sha256.len() != 64
            || !audio
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid audio declaration".into());
        }
        if audio.path.is_empty()
            || audio.path.len() > 4096
            || !Path::new(&audio.path)
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err("audio path must be repository relative".into());
        }
    }
    Ok(manifest)
}
pub fn load_audio(root: &Path, audio: &Audio) -> Result<Vec<f32>, Error> {
    let root = root.canonicalize()?;
    let path = root.join(&audio.path).canonicalize()?;
    if !path.starts_with(&root) {
        return Err("audio resolves outside repository".into());
    }
    decode_audio(&bounded_read(&path, MAX_AUDIO)?, audio)
}
pub fn decode_audio(bytes: &[u8], audio: &Audio) -> Result<Vec<f32>, Error> {
    if bytes.len() as u64 > MAX_AUDIO {
        return Err("audio exceeds size limit".into());
    }
    if digest(bytes) != audio.sha256 {
        return Err("audio SHA256 mismatch".into());
    }
    let mut wav = hound::WavReader::new(Cursor::new(bytes))?;
    let spec = wav.spec();
    if spec.channels != 1
        || spec.sample_rate != 16000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err("audio must be PCM16 mono at 16000 Hz".into());
    }
    if audio.frames == 0 || audio.frames > MAX_FRAMES || wav.duration() != audio.frames {
        return Err("audio frame count mismatch".into());
    }
    let samples: Vec<f32> = wav
        .samples::<i16>()
        .take(MAX_FRAMES as usize + 1)
        .map(|sample| sample.map(|value| f32::from(value) / 32768.0))
        .collect::<Result<_, _>>()?;
    if samples.len() != audio.frames as usize {
        return Err("audio payload length mismatch".into());
    }
    Ok(samples)
}
pub fn write_reports(output: &Path, results: &[u8], metadata: &[u8]) -> Result<(), Error> {
    let mut sidecar = output.as_os_str().to_os_string();
    sidecar.push(".metadata.json");
    let mut result_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(output)?;
    let mut metadata_file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&sidecar)
    {
        Ok(file) => file,
        Err(error) => {
            std::fs::remove_file(output)?;
            return Err(error.into());
        }
    };
    result_file.write_all(results)?;
    metadata_file.write_all(metadata)?;
    result_file.sync_all()?;
    metadata_file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_manifest_and_duplicate_ids_are_rejected() -> Result<(), Error> {
        let valid = serde_json::json!({"schemaVersion":1,"cases":[{"id":"de-01","language":"de","audio":{"path":"data/a.wav","sha256":"a".repeat(64),"frames":3,"sampleRate":16000}}]});
        assert!(parse_manifest(&serde_json::to_vec(&valid)?).is_ok());
        let mut duplicate = valid.clone();
        duplicate["cases"]
            .as_array_mut()
            .ok_or("cases")?
            .push(valid["cases"][0].clone());
        assert!(parse_manifest(&serde_json::to_vec(&duplicate)?).is_err());
        for (field, bad) in [
            ("id", serde_json::json!("")),
            ("id", serde_json::json!("ä")),
            ("language", serde_json::json!("auto")),
        ] {
            let mut value = valid.clone();
            value["cases"][0][field] = bad;
            assert!(parse_manifest(&serde_json::to_vec(&value)?).is_err());
        }
        for bad in ["../a.wav", "/a.wav", "data/../../a.wav"] {
            let mut value = valid.clone();
            value["cases"][0]["audio"]["path"] = serde_json::json!(bad);
            assert!(parse_manifest(&serde_json::to_vec(&value)?).is_err());
        }
        assert!(parse_manifest(b"{}").is_err());
        Ok(())
    }
    fn fixture() -> Result<(Vec<u8>, Audio), Error> {
        let mut cursor = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for sample in [i16::MIN, 0, i16::MAX] {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
        let bytes = cursor.into_inner();
        let audio = Audio {
            path: "input.wav".into(),
            sha256: digest(&bytes),
            frames: 3,
            sample_rate: 16000,
        };
        Ok((bytes, audio))
    }
    #[test]
    fn audio_hash_frame_count_and_actual_format_are_checked_on_the_same_bytes() -> Result<(), Error>
    {
        let (bytes, mut audio) = fixture()?;
        assert_eq!(decode_audio(&bytes, &audio)?, [-1.0, 0.0, 0.999_969_5]);
        audio.sha256 = "0".repeat(64);
        assert!(decode_audio(&bytes, &audio).is_err());
        audio.sha256 = digest(&bytes);
        audio.frames = 4;
        assert!(decode_audio(&bytes, &audio).is_err());
        audio.frames = 3;
        let mut corrupted = bytes.clone();
        corrupted[24..28].copy_from_slice(&8000u32.to_le_bytes());
        audio.sha256 = digest(&corrupted);
        assert!(decode_audio(&corrupted, &audio).is_err());
        let truncated = &bytes[..bytes.len() - 2];
        audio.sha256 = digest(truncated);
        assert!(decode_audio(truncated, &audio).is_err());
        Ok(())
    }
    #[test]
    fn manifest_rejects_oversize_and_all_unsupported_audio_declarations() -> Result<(), Error> {
        assert!(parse_manifest(&vec![b' '; MAX_MANIFEST as usize + 1]).is_err());
        let valid = serde_json::json!({"schemaVersion":1,"cases":[{"id":"case","language":"en","audio":{"path":"input.wav","sha256":"0".repeat(64),"frames":3,"sampleRate":16000},"reference":"ignored","class":"ignored"}]});
        assert!(parse_manifest(&serde_json::to_vec(&valid)?).is_ok());
        for (field, bad) in [
            ("frames", serde_json::json!(0)),
            ("frames", serde_json::json!(4800001)),
            ("sampleRate", serde_json::json!(44100)),
            ("sha256", serde_json::json!("A".repeat(64))),
            ("sha256", serde_json::json!("a".repeat(63))),
        ] {
            let mut value = valid.clone();
            value["cases"][0]["audio"][field] = bad;
            assert!(parse_manifest(&serde_json::to_vec(&value)?).is_err());
        }
        let mut value = valid.clone();
        value["schemaVersion"] = serde_json::json!(2);
        assert!(parse_manifest(&serde_json::to_vec(&value)?).is_err());
        value = valid.clone();
        value["cases"] = serde_json::json!([]);
        assert!(parse_manifest(&serde_json::to_vec(&value)?).is_err());
        value["cases"] = serde_json::Value::Array(
            (0..501)
                .map(|i| {
                    let mut case = valid["cases"][0].clone();
                    case["id"] = serde_json::json!(format!("case-{i}"));
                    case
                })
                .collect(),
        );
        assert!(parse_manifest(&serde_json::to_vec(&value)?).is_err());
        Ok(())
    }
    #[test]
    fn reads_are_bounded_and_exports_are_private_and_never_overwrite() -> Result<(), Error> {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("results.json");
        write_reports(&path, b"[]", b"{}")?;
        assert_eq!(std::fs::read(&path)?, b"[]");
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
        let sidecar = directory.path().join("results.json.metadata.json");
        assert_eq!(
            std::fs::metadata(&sidecar)?.permissions().mode() & 0o777,
            0o600
        );
        assert!(write_reports(&path, b"changed", b"changed").is_err());
        assert_eq!(std::fs::read(&path)?, b"[]");
        std::fs::remove_file(&path)?;
        assert!(write_reports(&path, b"changed", b"changed").is_err());
        assert!(!path.exists());
        assert_eq!(std::fs::read(&sidecar)?, b"{}");
        let (bytes, audio) = fixture()?;
        let wav = directory.path().join("input.wav");
        std::fs::write(&wav, bytes)?;
        assert_eq!(load_audio(directory.path(), &audio)?.len(), 3);
        assert!(bounded_read(&wav, 2).is_err());
        assert!(bounded_read(directory.path(), MAX_AUDIO).is_err());
        File::options()
            .write(true)
            .open(&wav)?
            .set_len(MAX_AUDIO + 1)?;
        assert!(load_audio(directory.path(), &audio).is_err());
        Ok(())
    }
    #[test]
    fn repository_relative_symlink_cannot_escape_the_corpus_root() -> Result<(), Error> {
        let root = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let (bytes, audio) = fixture()?;
        let target = outside.path().join("outside.wav");
        std::fs::write(&target, bytes)?;
        std::os::unix::fs::symlink(target, root.path().join("input.wav"))?;
        assert!(load_audio(root.path(), &audio).is_err());
        Ok(())
    }
}
