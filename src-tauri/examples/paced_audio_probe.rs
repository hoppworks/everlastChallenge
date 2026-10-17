//! Developer-only input pacing and audio-owner probe; no inference or L measurement.
#![forbid(unsafe_code)]
#[allow(
    dead_code,
    reason = "reuse the bounded, hash-verified corpus WAV input seam"
)]
#[path = "corpus_probe/input.rs"]
mod input;
#[path = "paced_audio_probe/runner.rs"]
mod runner;
#[path = "paced_audio_probe/scheduled.rs"]
mod scheduled;

use std::{io::Cursor, path::Path, time::Duration};

#[derive(Debug, PartialEq, Eq)]
enum Options {
    Legacy(std::path::PathBuf),
    PrintScheduling,
    Scheduling {
        wav: std::path::PathBuf,
        parameters: std::path::PathBuf,
        output: std::path::PathBuf,
    },
}
fn parse(arguments: &[std::ffi::OsString]) -> Result<Options, &'static str> {
    if arguments.len() == 1 && arguments[0] == "--print-scheduling-parameters" {
        return Ok(Options::PrintScheduling);
    }
    if arguments.len() == 1
        && !arguments[0].is_empty()
        && arguments[0].as_encoded_bytes().len() <= 4096
    {
        return Ok(Options::Legacy(arguments[0].clone().into()));
    }
    if arguments.iter().all(|a| {
        !a.is_empty()
            && a.as_encoded_bytes().len() <= 4096
            && !a.as_encoded_bytes().starts_with(b"--")
    }) && arguments.len() == 3
        && arguments[0] != arguments[2]
        && arguments[1] != arguments[2]
    {
        return Ok(Options::Scheduling {
            wav: arguments[0].clone().into(),
            parameters: arguments[1].clone().into(),
            output: arguments[2].clone().into(),
        });
    }
    Err(
        "usage: paced_audio_probe WAV | --print-scheduling-parameters | WAV PARAMETERS_JSON NEW_OUTPUT_JSON",
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os().skip(1).take(5).collect::<Vec<_>>();
    match parse(&arguments)? {
        Options::Legacy(wav) => legacy(&wav),
        Options::PrintScheduling => {
            println!(
                "{}",
                serde_json::to_string_pretty(&scheduled::parameters())?
            );
            Ok(())
        }
        Options::Scheduling {
            wav,
            parameters,
            output,
        } => scheduled::run(&wav, &parameters, &output),
    }
}
fn legacy(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = input::bounded_read(path, input::MAX_AUDIO)?;
    let wav = hound::WavReader::new(Cursor::new(&bytes))?;
    let declared = input::Audio {
        path: "authorized-local-fixture.wav".into(),
        sha256: input::digest(&bytes),
        frames: wav.duration(),
        sample_rate: wav.spec().sample_rate,
    };
    let samples = input::decode_audio(&bytes, &declared)?;
    let run = runner::run(&samples, Duration::from_secs(10));
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 3, "kind": "paced-audio-owner-probe", "passed": run.passed,
            "fixtureSha256": declared.sha256, "inputFrames": samples.len(),
            "sampleRate": 16000, "channels": 1, "chunkMillis": 20,
            "jitterToleranceNanos": 10000000,
            "waitStrategy": {"maximumSleepNanos":1000000,"finalSpinNanos":250000},
            "run": run,
            "probeSourceSha256": input::digest(include_bytes!("paced_audio_probe.rs")),
            "runnerSourceSha256": input::digest(include_bytes!("paced_audio_probe/runner.rs")),
            "audioSourceSha256": input::digest(include_bytes!("../src/audio.rs")),
            "replaySourceSha256": input::digest(include_bytes!("../src/audio/replay.rs")),
            "clockSourceSha256": input::digest(include_bytes!("../src/audio/replay/clock.rs")),
            "scheduleSourceSha256": input::digest(include_bytes!("../src/audio/replay/schedule.rs")),
            "sourceRevision": std::env::var("EVERLAST_BENCHMARK_REVISION").ok(),
            "L": null, "tText": null,
            "notes": [
                "This uses the production capture ring/DSP/limit/drain/flush owner, without CPAL.",
                "The feed clock never waits for DSP or inference and never releases a chunk before its planned audio time.",
                "All publication records survive observation failures, including finish timeout; an unobserved join is explicit.",
                "Timing failure invalidates the setup run. A 250-us final spin is input-clock overhead, not free scheduling.",
                "Stop is requested directly on CaptureHandle; there is no controller queue, inference, native paste or t_text here.",
                "This is audio-input validation, not the end-to-end L benchmark, microphone acceptance or a memory bound."
            ]
        }))?
    );
    if !run.passed {
        return Err("paced audio-owner probe failed; see complete timing evidence".into());
    }
    Ok(())
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    #[test]
    fn legacy_remains_explicit_and_new_scheduling_requires_all_three_paths()
    -> Result<(), &'static str> {
        assert_eq!(parse(&["a.wav".into()])?, Options::Legacy("a.wav".into()));
        assert_eq!(
            parse(&["--print-scheduling-parameters".into()])?,
            Options::PrintScheduling
        );
        assert_eq!(
            parse(&["a.wav".into(), "params.json".into(), "out.json".into()])?,
            Options::Scheduling {
                wav: "a.wav".into(),
                parameters: "params.json".into(),
                output: "out.json".into()
            }
        );
        for args in [
            vec![],
            vec!["a".into(), "b".into()],
            vec!["a".into(), "b".into(), "a".into()],
            vec!["--unknown".into(), "params".into(), "out".into()],
        ] {
            assert!(parse(&args).is_err());
        }
        assert!(parse(&["x".repeat(4097).into()]).is_err());
        Ok(())
    }
}
