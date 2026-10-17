//! Run orchestration and retained failures at the public measured-runtime seam.
use everlast_dictation::{
    audio::replay::clock::{FeedEvidence, feed},
    audio::replay::schedule::JITTER_TOLERANCE,
    controller::SessionSettings,
    runtime::measurement::{MeasuredRuntime, MeasuredTerminal, TimingSnapshot},
    transcription::Language,
};
use serde::Serialize;
use std::{
    thread,
    time::{Duration, Instant},
};
const WAIT: Duration = Duration::from_secs(120);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub id: String,
    pub warmup: bool,
    pub status: &'static str,
    pub error: Option<String>,
    pub session: Option<u64>,
    pub t_stop_offset_nanos: Option<u128>,
    pub t_text_offset_nanos: Option<u128>,
    pub stop_to_text_nanos: Option<u128>,
    pub controller_clock_matches_capture: bool,
    pub terminal: Option<String>,
    pub reference_match: Option<bool>,
    pub feed: Option<FeedEvidence>,
}
pub struct Output {
    pub observation: Observation,
    pub text: Option<String>,
}
fn observation(id: String, warmup: bool) -> Observation {
    Observation {
        id,
        warmup,
        status: "failed",
        error: None,
        session: None,
        t_stop_offset_nanos: None,
        t_text_offset_nanos: None,
        stop_to_text_nanos: None,
        controller_clock_matches_capture: false,
        terminal: None,
        reference_match: None,
        feed: None,
    }
}
pub fn unavailable(id: String, warmup: bool, error: String) -> Output {
    let mut observation = observation(id, warmup);
    observation.status = "unavailable";
    observation.error = Some(error);
    Output {
        observation,
        text: None,
    }
}
pub fn wait_ready(runtime: &MeasuredRuntime) -> Result<(), String> {
    let deadline = Instant::now() + WAIT;
    loop {
        let state = runtime.snapshot().map_err(|error| error.to_string())?;
        if state.model_ready {
            return Ok(());
        }
        if !state.running {
            return Err("runtime closed before model readiness".into());
        }
        if let Some(error) = state.error {
            return Err(error);
        }
        if Instant::now() >= deadline {
            return Err("model readiness observation timed out".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
}
fn offsets(row: &mut Observation, timing: TimingSnapshot, t0: Instant) {
    row.controller_clock_matches_capture = timing.t0 == Some(t0);
    row.t_stop_offset_nanos = timing
        .t_stop
        .and_then(|at| at.checked_duration_since(t0))
        .map(|d| d.as_nanos());
    row.t_text_offset_nanos = timing
        .t_text
        .and_then(|at| at.checked_duration_since(t0))
        .map(|d| d.as_nanos());
    row.stop_to_text_nanos = timing
        .t_text
        .zip(timing.t_stop)
        .and_then(|(text, stop)| text.checked_duration_since(stop))
        .map(|d| d.as_nanos());
    row.terminal = timing.terminal.map(|value| format!("{value:?}"));
}

pub fn one(
    runtime: &MeasuredRuntime,
    samples: &[f32],
    language: Language,
    id: String,
    warmup: bool,
    reference: Option<&str>,
) -> Output {
    let mut row = observation(id, warmup);
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        let mut reply = runtime.start(SessionSettings {
            device: None,
            language,
            generation: 1,
        })?;
        let deadline = Instant::now() + WAIT;
        let session = loop {
            match reply.try_recv() {
                Ok(result) => break result?,
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
                    if Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(1))
                }
                _ => {
                    return Err("measured start acknowledgement unavailable before deadline".into());
                }
            }
        };
        row.session = Some(session);
        let mut source = runtime.take_input(WAIT)?;
        let t0 = source.started;
        row.feed = Some(feed(&mut source, samples)?);
        let stopped = runtime.stop(session)?;
        drop(source.port);
        let deadline = Instant::now() + WAIT;
        let timing = loop {
            let timing = runtime.timing()?;
            if timing.session != Some(session) {
                return Err("measurement session identity changed".into());
            }
            offsets(&mut row, timing, t0);
            if timing.terminal.is_some() {
                break timing;
            }
            let state = runtime.snapshot()?;
            if !state.running {
                return Err("runtime closed before final text".into());
            }
            if Instant::now() >= deadline {
                return Err("controller final-text observation timed out".into());
            }
            thread::sleep(Duration::from_millis(1));
        };
        if timing.t_stop != Some(stopped) || !row.controller_clock_matches_capture {
            return Err("host/controller/capture monotone clock identity mismatch".into());
        }
        if timing.terminal != Some(MeasuredTerminal::Local) || timing.t_text.is_none() {
            return Err("run ended without accepted nonempty final controller text".into());
        }
        let state = runtime.snapshot()?;
        state
            .session
            .text
            .ok_or_else(|| "controller final text is missing".into())
    })();
    match result {
        Ok(text) => {
            row.reference_match = reference.map(|expected| text.trim() == expected.trim());
            let stop_valid =
                row.feed
                    .as_ref()
                    .zip(row.t_stop_offset_nanos)
                    .is_some_and(|(input, stop)| {
                        let planned = input.planned_stop_offset.as_nanos();
                        input.timing_valid
                            && stop >= planned
                            && stop - planned <= JITTER_TOLERANCE.as_nanos()
                    });
            if !stop_valid {
                row.status = "invalid_setup";
                row.error = Some("input or stop exceeded the fixed timing budget".into());
            } else if row.reference_match == Some(false) {
                row.status = "reference_mismatch";
                row.error = Some("transcript differs from the supplied reference".into());
            } else if row.stop_to_text_nanos.is_none() {
                row.error = Some("final text timestamp precedes stop or is missing".into());
            } else {
                row.status = "ok";
            }
            Output {
                observation: row,
                text: Some(text),
            }
        }
        Err(error) => {
            row.error = Some(error.to_string());
            if let Some(session) = row.session
                && let Err(cancel_error) = runtime.cancel(session)
            {
                row.error = Some(format!(
                    "{}; cancellation failed: {cancel_error}",
                    row.error.as_deref().unwrap_or("run failed")
                ));
            }
            Output {
                observation: row,
                text: None,
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cleanup {
    pub joined: bool,
    pub error: Option<String>,
    pub elapsed_micros: u128,
}
pub fn cleanup(runtime: MeasuredRuntime) -> Cleanup {
    let started = Instant::now();
    let (send, receive) = crossbeam_channel::bounded(1);
    let join = match thread::Builder::new()
        .name("paced-probe-join".into())
        .spawn(move || {
            if send.send(runtime.join()).is_err() { /* observer stopped */ }
        }) {
        Ok(join) => join,
        Err(error) => {
            return Cleanup {
                joined: false,
                error: Some(format!(
                    "cleanup observer could not start; join unverified: {error}"
                )),
                elapsed_micros: started.elapsed().as_micros(),
            };
        }
    };
    let result = match receive.recv_timeout(WAIT) {
        Ok(result) => result,
        Err(error) => {
            return Cleanup {
                joined: false,
                error: Some(format!(
                    "cleanup observation failed; join unverified: {error}"
                )),
                elapsed_micros: started.elapsed().as_micros(),
            };
        }
    };
    let deadline = Instant::now() + WAIT;
    while !join.is_finished() {
        if Instant::now() >= deadline {
            return Cleanup {
                joined: false,
                error: Some("cleanup thread exit timed out; join unverified".into()),
                elapsed_micros: started.elapsed().as_micros(),
            };
        }
        thread::sleep(Duration::from_millis(1));
    }
    if join.join().is_err() {
        return Cleanup {
            joined: false,
            error: Some("cleanup observer panicked".into()),
            elapsed_micros: started.elapsed().as_micros(),
        };
    }
    match result {
        Ok(()) => Cleanup {
            joined: true,
            error: None,
            elapsed_micros: started.elapsed().as_micros(),
        },
        Err(error) => Cleanup {
            joined: false,
            error: Some(error.to_string()),
            elapsed_micros: started.elapsed().as_micros(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dictation_core::audio::AudioFormat;
    use everlast_dictation::{
        inference_service::{Decoder, InferenceService},
        transcription::InferenceError,
    };
    struct Fixed;
    impl Decoder for Fixed {
        fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
            if audio != [0.125; 320] {
                return Err(InferenceError::InvalidAudio);
            }
            Ok("controlled words".into())
        }
    }
    #[test]
    fn complete_run_uses_controller_timestamps_and_preserves_reference_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let measured = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Fixed)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_ready(&measured)?;
        let output = one(
            &measured,
            &[0.125; 320],
            Language::German,
            "controlled".into(),
            false,
            Some("controlled words"),
        );
        let ended = cleanup(measured);
        assert!(ended.joined);
        assert_eq!(output.text.as_deref(), Some("controlled words"));
        assert_eq!(output.observation.reference_match, Some(true));
        assert_eq!(output.observation.terminal.as_deref(), Some("Local"));
        assert!(output.observation.controller_clock_matches_capture);
        assert!(output.observation.stop_to_text_nanos.is_some());
        // Scheduling errors remain invalid rather than relaxing a fixed budget.
        assert!(matches!(output.observation.status, "ok" | "invalid_setup"));
        Ok(())
    }
}
