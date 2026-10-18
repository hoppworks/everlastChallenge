//! Developer-only prepared-prefix diagnostic; no text export or production flag.
#![forbid(unsafe_code)]
#![recursion_limit = "256"]
#[path = "prefix_cache_probe/contract.rs"]
mod contract;
#[allow(
    dead_code,
    reason = "reuse bounded verified WAV/manifest input, never transcript export"
)]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "prefix_cache_probe/runner.rs"]
mod runner;
#[path = "prefix_cache_probe/sources.rs"]
mod sources;

use everlast_dictation::{
    inference_service::InferenceService,
    model::{REFERENCE, VAD},
    transcription::{Language, prefix_cache::PrefixCacheDecoder},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::Arc,
    time::Instant,
};
type Error = Box<dyn std::error::Error>;
const MAX_REPORT: usize = 1024 * 1024;

fn check_new(path: &Path) -> Result<(), Error> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err("report destination already exists".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
fn publish(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > MAX_REPORT {
        return Err("diagnostic report exceeded1MiB".into());
    }
    check_new(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    staged.persist_noclobber(path)?;
    Ok(())
}
fn binding(path: &Path, limit: u64) -> Result<Value, Error> {
    let before = std::fs::symlink_metadata(path)?;
    if !before.is_file() || before.len() > limit {
        return Err("bound file is not bounded regular data".into());
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    use std::os::unix::fs::MetadataExt;
    if before.dev() != opened.dev() || before.ino() != opened.ino() || before.len() != opened.len()
    {
        return Err("bound file changed before reading".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or("bound file byte overflow")?;
        if total > limit {
            return Err("bound file grew beyond limit".into());
        }
        hash.update(&buffer[..read]);
    }
    let after = file.metadata()?;
    if total != before.len()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err("bound file changed while reading".into());
    }
    Ok(
        json!({"path":path.to_str().ok_or("bound path is not UTF8")?,"bytes":total,
        "sha256":format!("{:x}",hash.finalize()),"mode":format!("0o{:o}",after.permissions().mode()&0o777)}),
    )
}
fn shape(manifest: &input::Manifest) -> Result<(), Error> {
    if !matches!(manifest.cases.len(), 2 | 4) {
        return Err("manifest requires one or two consecutive prefix/whole pairs".into());
    }
    for pair in manifest.cases.chunks_exact(2) {
        if pair[0].language != pair[1].language || pair[0].audio.frames > pair[1].audio.frames {
            return Err("declared prefix/whole language or frame relation differs".into());
        }
    }
    Ok(())
}
fn main() -> Result<(), Error> {
    let args = std::env::args_os().skip(1).take(4).collect::<Vec<_>>();
    let options = contract::Options::parse(&args)?;
    check_new(&options.output)?;
    let parameters = sources::parameters()?;
    let manifest_bytes = input::bounded_read(&options.manifest, input::MAX_MANIFEST)?;
    let manifest = input::parse_manifest(&manifest_bytes)?;
    shape(&manifest)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root missing")?;
    // Validate every actual input relation before initializing any model.
    let mut pairs = Vec::with_capacity(manifest.cases.len() / 2);
    let mut inputs = Vec::with_capacity(manifest.cases.len());
    for cases in manifest.cases.chunks_exact(2) {
        let prefix = input::load_audio(root, &cases[0].audio)?;
        let whole = input::load_audio(root, &cases[1].audio)?;
        contract::validate_pair(&cases[0].language, &cases[1].language, &prefix, &whole)?;
        for (case, samples) in [(&cases[0], &prefix), (&cases[1], &whole)] {
            inputs.push(json!({"id":case.id,"language":case.language,"audio":{"path":case.audio.path,
                "sha256":case.audio.sha256,"frames":case.audio.frames,"sampleRate":case.audio.sample_rate},
                "inputPcmSha256":runner::pcm_sha(samples)}));
        }
        pairs.push(runner::PairInput {
            prefix_id: cases[0].id.clone(),
            whole_id: cases[1].id.clone(),
            language: if cases[0].language == "de" {
                Language::German
            } else {
                Language::English
            },
            prefix: Arc::new(prefix),
            whole: Arc::new(whole),
        });
    }
    let binary = std::env::current_exe()
        .map_err(Error::from)
        .and_then(|path| binding(&path, 1024 * 1024 * 1024));
    let model = binding(&options.model, REFERENCE.bytes());
    let vad_path = options.model.with_file_name(VAD.file_name());
    let vad = binding(&vad_path, VAD.bytes());
    let mut blocked = if binary.is_err() || model.is_err() || vad.is_err() {
        Some("diagnostic binary/model binding failed")
    } else {
        None
    };
    let mut reports = Vec::with_capacity(pairs.len());
    for (index, pair) in pairs.iter().enumerate() {
        let started = Instant::now();
        let mut run = if let Some(error) = blocked {
            runner::unavailable(pair, index, error)
        } else {
            match PrefixCacheDecoder::load(&options.model, REFERENCE) {
                Ok(decoder) => {
                    let (decoder, receiver) = runner::observe(decoder);
                    match InferenceService::with_decoder(decoder) {
                        Ok(service) => runner::run_pair(service, receiver, pair, index),
                        Err(_) => {
                            let mut run = runner::unavailable(
                                pair,
                                index,
                                "inference worker initialization failed",
                            );
                            run.rows[0].status = "failed";
                            run
                        }
                    }
                }
                Err(_) => {
                    let mut run = runner::unavailable(
                        pair,
                        index,
                        "verified reference ASR/VAD initializer failed",
                    );
                    run.rows[0].status = "failed";
                    run
                }
            }
        };
        if blocked.is_some() && index == 0 {
            run.rows[0].status = "failed";
        }
        if !run.joined || run.rows.iter().any(|r| r.status != "ok") {
            blocked = Some("previous pair failed; no further model work");
        }
        reports.push(
            json!({"pairIndex":index,"pairWallNanos":started.elapsed().as_nanos(),"run":run}),
        );
    }
    let passed = blocked.is_none();
    let document = json!({"schemaVersion":1,"kind":"prepared-prefix-cache-diagnostic","processId":std::process::id(),
        "passed":passed,"qualified":false,"L":null,"debugAssertions":cfg!(debug_assertions),
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),"sourceHashes":sources::hashes(),
        "parameters":parameters,"binary":binary.ok(),"modelDefinition":REFERENCE,"model":model.ok(),"vadDefinition":VAD,"vad":vad.ok(),
        "manifest":{"path":options.manifest,"sha256":input::digest(&manifest_bytes),"bytes":manifest_bytes.len()},
        "inputs":inputs,"pairs":reports,
        "limits":["no raw transcript or PCM export; hashes/byte lengths are not semantic quality",
            "model/context initialized on CLI main then moved to actual serial worker; no production cold-start equivalence",
            "120s observation budgets do not establish hard native cancellation; unverified cleanup blocks all later work",
            "not paced, not microphone/native delivery; no stop-to-text L, quality or resource qualification"]});
    publish(&options.output, &serde_json::to_vec_pretty(&document)?)?;
    if passed {
        Ok(())
    } else {
        Err("prefix cache diagnostic failed; bounded private evidence retained".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_atomic_report_never_replaces_existing_evidence() -> Result<(), Error> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("new.json");
        publish(&path, b"{\"shaOnly\":true}")?;
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
        assert!(publish(&path, b"changed").is_err());
        assert_eq!(std::fs::read(&path)?, b"{\"shaOnly\":true}");
        assert!(publish(&dir.path().join("oversize.json"), &vec![0; MAX_REPORT + 1]).is_err());
        Ok(())
    }
    #[test]
    fn manifest_pairs_are_bounded_and_ordered_before_model_loading() -> Result<(), Error> {
        fn case(id: usize, language: &str, frames: u32) -> Value {
            json!({"id":format!("case{id}"),"language":language,"audio":{"path":format!("artifacts/{id}.wav"),"sha256":"a".repeat(64),"frames":frames,"sampleRate":16000}})
        }
        let valid = json!({"schemaVersion":1,"cases":[case(1,"de",16000),case(2,"de",32000)]});
        shape(&input::parse_manifest(&serde_json::to_vec(&valid)?)?)?;
        for count in [1, 3, 5, 6] {
            let doc = json!({"schemaVersion":1,"cases":(0..count).map(|n|case(n,"de",16000)).collect::<Vec<_>>()});
            assert!(shape(&input::parse_manifest(&serde_json::to_vec(&doc)?)?).is_err());
        }
        let mut wrong = valid.clone();
        wrong["cases"][1]["language"] = json!("en");
        assert!(shape(&input::parse_manifest(&serde_json::to_vec(&wrong)?)?).is_err());
        wrong = valid;
        wrong["cases"][0]["audio"]["frames"] = json!(48000);
        assert!(shape(&input::parse_manifest(&serde_json::to_vec(&wrong)?)?).is_err());
        Ok(())
    }
}
