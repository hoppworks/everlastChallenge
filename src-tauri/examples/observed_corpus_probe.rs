//! Explicit private diagnostic export from the common Engine path; not runtime L.
#![forbid(unsafe_code)]
#[allow(
    dead_code,
    reason = "reuse the bounded hash-verified corpus input and private export seam"
)]
#[path = "corpus_probe/input.rs"]
mod input;

use everlast_dictation::{
    model::{REFERENCE, SMALL},
    transcription::{Engine, Language},
};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).take(5).collect::<Vec<_>>();
    if args.len() != 4
        || args
            .iter()
            .any(|arg| arg.is_empty() || arg.as_encoded_bytes().len() > 4096)
    {
        return Err(
            "usage: observed_corpus_probe MODEL reference|small MANIFEST NEW_OUTPUT".into(),
        );
    }
    let definition = match args[1].to_str() {
        Some("reference") => REFERENCE,
        Some("small") => SMALL,
        _ => return Err("model selection must be reference or small".into()),
    };
    let bytes = input::bounded_read(Path::new(&args[2]), input::MAX_MANIFEST)?;
    let manifest = input::parse_manifest(&bytes)?;
    if manifest.cases.len() > 4 {
        return Err("native observation is limited to four diagnostic cases".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root unavailable")?;
    // This developer process's main thread is the sole serial model owner.
    // Engine verifies both ASR and VAD files; no Tauri or microphone is started.
    let load_start = Instant::now();
    let mut engine = Engine::load(Path::new(&args[0]), definition);
    let load_millis = load_start.elapsed().as_millis();
    let mut rows = Vec::<Value>::with_capacity(manifest.cases.len());
    let mut failed = false;
    for case in manifest.cases {
        let start = Instant::now();
        let result = match engine.as_mut() {
            Ok(engine) => input::load_audio(root, &case.audio)
                .map_err(|error| error.to_string())
                .and_then(|samples| {
                    engine
                        .transcribe_observed(
                            &samples,
                            if case.language == "de" {
                                Language::German
                            } else {
                                Language::English
                            },
                        )
                        .map_err(|error| error.to_string())
                }),
            Err(error) => Err(error.to_string()),
        };
        let row = match result {
            Ok(result) => {
                let observation = result.observation;
                let segments = observation
                    .segments
                    .into_iter()
                    .map(|segment| {
                        json!({
                            "index": segment.index,
                            "startCentiseconds": segment.start_centiseconds,
                            "endCentiseconds": segment.end_centiseconds,
                            "noSpeechProbability": segment.no_speech_probability,
                            "text": segment.text,
                        })
                    })
                    .collect::<Vec<_>>();
                json!({"id":case.id,"status":"ok","text":result.text,"error":null,
                "jobMillis":start.elapsed().as_millis(),
                "observation":{
                    "inputFrames":observation.input_frames,
                    "selectedRange":observation.selected_range.map(|range|json!({"start":range.start,"end":range.end})),
                    "selectedFrames":observation.selected_frames,
                    "asrInputFrames":observation.asr_input_frames,
                    "paddedFrames":observation.padded_frames,
                    "segments":segments,
                }})
            }
            Err(error) => {
                failed = true;
                json!({"id":case.id,"status":"failed","text":null,"observation":null,
                    "error":error,"jobMillis":start.elapsed().as_millis()})
            }
        };
        rows.push(row);
    }
    drop(engine);
    let metadata = json!({
        "schemaVersion":1,"kind":"common-engine-accepted-native-segment-diagnostic",
        "sourceRevision":std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
        "manifestSha256":input::digest(&bytes),
        "model":{"id":definition.id(),"sha256":definition.sha256()},
        "loadMillis":load_millis,"failed":failed,"L":null,"cases":rows,
        "probeSourceSha256":input::digest(include_bytes!("observed_corpus_probe.rs")),
        "engineSourceSha256":input::digest(include_bytes!("../src/transcription.rs")),
        "observationSourceSha256":input::digest(include_bytes!("../src/transcription/observation.rs")),
        "limits":[
            "Same production Engine preparation, parameters and segment join; direct serial owner, no InferenceService/controller/audio pipeline.",
            "Only accepted native segments after full(); missing or rejected windows, seek, prompt history and fallback temperature are unobserved.",
            "Native centisecond times with no_timestamps=true are not word alignments; segment NSP may share a window value.",
            "At most four cases, 4096 segments and 256 KiB raw segment text per case; failures remain explicit.",
            "Private explicit exports only; no model-independent timing, real-time L, microphone or native paste qualification."
        ]
    });
    input::write_reports(
        Path::new(&args[3]),
        &serde_json::to_vec_pretty(&quality_rows(&rows))?,
        &serde_json::to_vec_pretty(&metadata)?,
    )?;
    println!("observed corpus: {} cases, failures={failed}", rows.len());
    if failed {
        return Err("native observation recorded failures; inspect private exports".into());
    }
    Ok(())
}

fn quality_rows(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|row| json!({"id":row["id"],"status":row["status"],"text":row["text"]}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_export_keeps_exact_three_field_contract_and_failure_status() {
        let rows = vec![
            json!({"id":"ok","status":"ok","text":" Kept. ","error":null,"observation":{"segments":[]}}),
            json!({"id":"failed","status":"failed","text":null,"error":"invalid native metadata","observation":null}),
        ];
        assert_eq!(
            quality_rows(&rows),
            vec![
                json!({"id":"ok","status":"ok","text":" Kept. "}),
                json!({"id":"failed","status":"failed","text":null}),
            ]
        );
        assert_eq!(rows[1]["error"], "invalid native metadata");
    }
}
