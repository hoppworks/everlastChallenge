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
    pub qos_entered: bool,
    pub qos_finished: bool,
    pub qos_error: Option<String>,
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
        qos_entered: false,
        qos_finished: false,
        qos_error: None,
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

trait PublisherScope {
    fn finish(self) -> Result<(), String>;
}

impl PublisherScope for macos_integration::qos::PublisherQos {
    fn finish(self) -> Result<(), String> {
        self.finish().map_err(|error| error.to_string())
    }
}

pub fn one(
    runtime: &MeasuredRuntime,
    samples: &[f32],
    language: Language,
    id: String,
    warmup: bool,
    reference: Option<&str>,
) -> Output {
    one_scoped(runtime, samples, language, id, warmup, reference, || {
        macos_integration::qos::PublisherQos::enter().map_err(|error| error.to_string())
    })
}

fn one_scoped<Q: PublisherScope>(
    runtime: &MeasuredRuntime,
    samples: &[f32],
    language: Language,
    id: String,
    warmup: bool,
    reference: Option<&str>,
    enter: impl FnOnce() -> Result<Q, String>,
) -> Output {
    let mut row = observation(id, warmup);
    let mut qos = match enter() {
        Ok(scope) => {
            row.qos_entered = true;
            Some(scope)
        }
        Err(error) => {
            row.status = "invalid_setup";
            row.qos_error = Some(error.clone());
            row.error = Some(error);
            return Output {
                observation: row,
                text: None,
            };
        }
    };
    let mut result = (|| -> Result<String, Box<dyn std::error::Error>> {
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
        row.t_stop_offset_nanos = stopped.checked_duration_since(t0).map(|d| d.as_nanos());
        if let Some(scope) = qos.take() {
            match scope.finish() {
                Ok(()) => row.qos_finished = true,
                Err(error) => {
                    row.qos_error = Some(error.clone());
                    return Err(error.into());
                }
            }
        }
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
    // Error paths drop the source port while unwinding the closure, then end
    // the override before cancellation/observer work. Preserve both failures.
    if let Some(scope) = qos.take() {
        match scope.finish() {
            Ok(()) => row.qos_finished = true,
            Err(error) => {
                row.qos_error = Some(error.clone());
                result = Err(match result {
                    Err(original) => format!("{original}; {error}").into(),
                    Ok(_) => error.into(),
                });
            }
        }
    }
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
            if row.qos_error.is_some() {
                row.status = "invalid_setup";
            }
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
    struct FakeScope {
        ends: std::rc::Rc<std::cell::Cell<u32>>,
        failure: Option<String>,
    }
    impl PublisherScope for FakeScope {
        fn finish(self) -> Result<(), String> {
            self.ends.set(self.ends.get() + 1);
            self.failure.map_or(Ok(()), Err)
        }
    }
    fn controlled_runtime() -> Result<MeasuredRuntime, Box<dyn std::error::Error>> {
        let measured = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Fixed)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_ready(&measured)?;
        Ok(measured)
    }
    #[test]
    fn qos_enter_failure_prevents_session_start_and_is_retained_as_invalid_setup()
    -> Result<(), Box<dyn std::error::Error>> {
        let measured = controlled_runtime()?;
        let output = one_scoped::<FakeScope>(
            &measured,
            &[0.125; 320],
            Language::German,
            "enter-failed".into(),
            false,
            None,
            || Err("controlled QoS enter failure".into()),
        );
        let ended = cleanup(measured);
        assert!(ended.joined);
        assert_eq!(output.observation.status, "invalid_setup");
        assert_eq!(output.observation.session, None);
        assert!(
            output
                .observation
                .error
                .as_deref()
                .is_some_and(|error| error.contains("controlled QoS enter failure"))
        );
        assert!(output.text.is_none());
        Ok(())
    }
    #[test]
    fn qos_finish_failure_preserves_feed_and_stop_but_never_claims_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let measured = controlled_runtime()?;
        let ends = std::rc::Rc::new(std::cell::Cell::new(0));
        let output = one_scoped(
            &measured,
            &[0.125; 320],
            Language::German,
            "finish-failed".into(),
            false,
            None,
            || {
                Ok(FakeScope {
                    ends: std::rc::Rc::clone(&ends),
                    failure: Some("controlled QoS finish failure".into()),
                })
            },
        );
        let ended = cleanup(measured);
        assert!(ended.joined);
        assert_eq!(ends.get(), 1);
        assert_eq!(output.observation.status, "invalid_setup");
        assert!(output.observation.feed.is_some());
        assert!(output.observation.t_stop_offset_nanos.is_some());
        assert!(
            output
                .observation
                .error
                .as_deref()
                .is_some_and(|error| error.contains("controlled QoS finish failure"))
        );
        assert!(output.text.is_none());
        Ok(())
    }
    #[test]
    fn early_feed_error_still_finishes_qos_and_preserves_both_failure_reasons()
    -> Result<(), Box<dyn std::error::Error>> {
        let measured = controlled_runtime()?;
        let ends = std::rc::Rc::new(std::cell::Cell::new(0));
        let output = one_scoped(
            &measured,
            &[],
            Language::German,
            "feed-and-finish-failed".into(),
            false,
            None,
            || {
                Ok(FakeScope {
                    ends: std::rc::Rc::clone(&ends),
                    failure: Some("controlled QoS finish failure".into()),
                })
            },
        );
        let ended = cleanup(measured);
        assert!(ended.joined);
        assert_eq!(ends.get(), 1);
        assert_eq!(output.observation.status, "invalid_setup");
        assert!(
            output
                .observation
                .error
                .as_deref()
                .is_some_and(|error| error.contains("Replay frame count")
                    && error.contains("controlled QoS finish failure"))
        );
        assert!(output.text.is_none());
        Ok(())
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
        let ends = std::rc::Rc::new(std::cell::Cell::new(0));
        let output = one_scoped(
            &measured,
            &[0.125; 320],
            Language::German,
            "controlled".into(),
            false,
            Some("controlled words"),
            || {
                Ok(FakeScope {
                    ends: std::rc::Rc::clone(&ends),
                    failure: None,
                })
            },
        );
        let ended = cleanup(measured);
        assert!(ended.joined);
        assert_eq!(ends.get(), 1);
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
