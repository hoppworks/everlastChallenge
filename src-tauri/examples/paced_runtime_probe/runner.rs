#[path = "../shared/audio_context.rs"]
pub mod audio_context;
// Run orchestration and retained failures at the public measured-runtime seam.
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
#[path = "publisher.rs"]
mod publisher;

const WAIT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ExpectedOutcome {
    Local,
    Empty,
}

#[derive(Clone, Copy)]
struct Expectation<'a> {
    outcome: ExpectedOutcome,
    reference: Option<&'a str>,
    context: Option<&'a audio_context::Receipts>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub id: String,
    pub warmup: bool,
    pub expected_outcome: ExpectedOutcome,
    pub t_terminal_offset_nanos: Option<u128>,
    pub stop_to_terminal_nanos: Option<u128>,
    pub engine_elapsed_nanos: Option<u128>,
    pub engine_input_frames: Option<usize>,
    pub engine_rtf: Option<f64>,
    pub engine_rtf_unavailable_reason: Option<String>,
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
    pub publisher_joined: Option<bool>,
    pub publisher: Option<serde_json::Value>,
    pub audio_context: Option<serde_json::Value>,
}
pub struct Output {
    pub observation: Observation,
    pub text: Option<String>,
}
fn observation(id: String, warmup: bool) -> Observation {
    Observation {
        id,
        warmup,
        expected_outcome: ExpectedOutcome::Local,
        t_terminal_offset_nanos: None,
        stop_to_terminal_nanos: None,
        engine_elapsed_nanos: None,
        engine_input_frames: None,
        engine_rtf: None,
        engine_rtf_unavailable_reason: Some("no accepted decoder timing".into()),
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
        publisher_joined: None,
        publisher: None,
        audio_context: None,
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
    row.t_terminal_offset_nanos = timing
        .t_terminal
        .and_then(|at| at.checked_duration_since(t0))
        .map(|d| d.as_nanos());
    row.stop_to_terminal_nanos = timing
        .t_terminal
        .zip(timing.t_stop)
        .and_then(|(end, stop)| end.checked_duration_since(stop))
        .map(|d| d.as_nanos());
    row.engine_elapsed_nanos = timing.decode_timing.map(|value| value.elapsed.as_nanos());
    row.engine_input_frames = timing.decode_timing.map(|value| value.input_frames);
    row.engine_rtf = timing.decode_timing.and_then(|value| value.rtf());
    row.engine_rtf_unavailable_reason = match timing.decode_timing {
        None => Some("no accepted decoder timing; no RTF inferred from Empty".into()),
        Some(value) if value.input_frames == 0 => {
            Some("decoder received zero frames; RTF denominator is zero".into())
        }
        Some(_) => None,
    };
    row.terminal = timing.terminal.map(|value| format!("{value:?}"));
}

fn validate_engine_timing(
    timing: Option<everlast_dictation::inference_service::DecodeTiming>,
    frames: usize,
    expected: ExpectedOutcome,
) -> Result<(), &'static str> {
    match timing {
        Some(value) if value.input_frames != frames => {
            Err("accepted Engine input frame count differs from complete fixture")
        }
        None if expected == ExpectedOutcome::Local => {
            Err("accepted Local result lacks actual Engine timing")
        }
        _ => Ok(()),
    }
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
    one_expected(
        runtime,
        samples,
        language,
        id,
        warmup,
        reference,
        ExpectedOutcome::Local,
    )
}

pub fn one_expected(
    runtime: &MeasuredRuntime,
    samples: &[f32],
    language: Language,
    id: String,
    warmup: bool,
    reference: Option<&str>,
    expected: ExpectedOutcome,
) -> Output {
    one_scoped(
        runtime,
        samples,
        language,
        id,
        warmup,
        Expectation {
            outcome: expected,
            reference,
            context: None,
        },
        || macos_integration::qos::PublisherQos::enter().map_err(|error| error.to_string()),
    )
}

/// Explicit frozen TC alternative. Samples are shared once per loaded fixture;
/// Runtime remains owned by the normal parent and is never captured by the child.
pub fn one_scheduled(
    runtime: &MeasuredRuntime,
    samples: &std::sync::Arc<Vec<f32>>,
    language: Language,
    id: String,
    warmup: bool,
    reference: Option<&str>,
    expected: ExpectedOutcome,
) -> Output {
    one_scheduled_with(
        runtime,
        samples,
        language,
        id,
        warmup,
        Expectation {
            outcome: expected,
            reference,
            context: None,
        },
        || publisher::Native::spawn(std::sync::Arc::clone(samples)),
    )
}
pub struct ContextRun<'a> {
    pub receipts: &'a audio_context::Receipts,
    pub reference: Option<&'a str>,
    pub outcome: ExpectedOutcome,
}
/// Same scheduled orchestration, with receipt admission after the actual Start ACK.
pub fn one_context(
    runtime: &MeasuredRuntime,
    samples: &std::sync::Arc<Vec<f32>>,
    language: Language,
    id: String,
    warmup: bool,
    context: ContextRun<'_>,
) -> Output {
    one_scheduled_with(
        runtime,
        samples,
        language,
        id,
        warmup,
        Expectation {
            outcome: context.outcome,
            reference: context.reference,
            context: Some(context.receipts),
        },
        || publisher::Native::spawn(std::sync::Arc::clone(samples)),
    )
}

fn one_scheduled_with<P: publisher::Publisher>(
    runtime: &MeasuredRuntime,
    samples: &std::sync::Arc<Vec<f32>>,
    language: Language,
    id: String,
    warmup: bool,
    expectation: Expectation<'_>,
    make: impl FnOnce() -> Result<P, String>,
) -> Output {
    let mut row = observation(id, warmup);
    row.expected_outcome = expectation.outcome;
    if expectation.outcome == ExpectedOutcome::Empty && expectation.reference.is_some() {
        row.error = Some("Empty expectation cannot have a text reference".into());
        return Output {
            observation: row,
            text: None,
        };
    }
    if samples.is_empty() || samples.len() > 4_800_000 {
        row.error = Some("scheduled fixture outside 1..4800000 mono frames".into());
        return Output {
            observation: row,
            text: None,
        };
    }
    let mut publisher = match make() {
        Ok(p) => p,
        Err(error) => {
            row.status = "invalid_setup";
            row.error = Some(error);
            return Output {
                observation: row,
                text: None,
            };
        }
    };
    // Feed duration plus the existing observer budget, not a 120s cap on a
    // legitimate 300s feed. Start/input waits retain their own bounded budget.
    let budget = Duration::from_secs((samples.len() as u64).div_ceil(16000)) + WAIT;
    let published = publisher::orchestrate(
        &mut publisher,
        || {
            let (session, source) =
                start_input(runtime, language, &mut row).map_err(|e| e.to_string())?;
            if let Some(receipts) = expectation.context
                && let Err(error) = receipts.arm(session, 1, session, samples.len(), language)
            {
                let cancelled = runtime.cancel(session).err().map(|e| e.to_string());
                return Err(format!("{error}; cancellation error: {cancelled:?}"));
            }
            Ok(publisher::Input {
                source,
                stop: runtime.stop_handle(session),
            })
        },
        budget,
    );
    row.publisher_joined = Some(published.joined);
    row.publisher = published.policy;
    row.feed = published.feed;
    let result = if !published.joined {
        Err("publisher join was not confirmed".into())
    } else if let Some((t0, stopped)) = published.t0.zip(published.stopped) {
        row.t_stop_offset_nanos = stopped.checked_duration_since(t0).map(|d| d.as_nanos());
        observe_terminal(
            runtime,
            &mut row,
            t0,
            stopped,
            samples.len(),
            expectation.outcome,
        )
    } else {
        Err("publisher did not return complete feed/stop evidence".into())
    };
    let mut output = finish_result(runtime, row, result, expectation);
    if !published.errors.is_empty() {
        let mut errors = published.errors;
        if let Some(error) = output.observation.error.take() {
            errors.push(error);
        }
        output.observation.status = "invalid_setup";
        output.observation.error = Some(errors.join("; "));
    }
    if let Some(receipts) = expectation.context {
        let collection = receipts.collect(output.observation.session, samples.len(), language);
        if let Some(error) = &collection.error {
            if output.observation.status == "ok" {
                output.observation.status = "failed";
            }
            output.observation.error = Some(match output.observation.error.take() {
                Some(old) => format!("{old}; {error}"),
                None => error.clone(),
            });
        }
        output.observation.audio_context = Some(serde_json::json!(collection));
    }
    output
}

fn one_scoped<Q: PublisherScope>(
    runtime: &MeasuredRuntime,
    samples: &[f32],
    language: Language,
    id: String,
    warmup: bool,
    expectation: Expectation<'_>,
    enter: impl FnOnce() -> Result<Q, String>,
) -> Output {
    let mut row = observation(id, warmup);
    row.expected_outcome = expectation.outcome;
    if expectation.outcome == ExpectedOutcome::Empty && expectation.reference.is_some() {
        row.error = Some("Empty expectation cannot have a text reference".into());
        return Output {
            observation: row,
            text: None,
        };
    }
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
        let (session, mut source) = start_input(runtime, language, &mut row)?;
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
        observe_terminal(
            runtime,
            &mut row,
            t0,
            stopped,
            samples.len(),
            expectation.outcome,
        )
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
    finish_result(runtime, row, result, expectation)
}

fn start_input(
    runtime: &MeasuredRuntime,
    language: Language,
    row: &mut Observation,
) -> Result<(u64, everlast_dictation::audio::replay::ReplayInput), Box<dyn std::error::Error>> {
    let mut reply = runtime.start(SessionSettings {
        device: None,
        language,
        generation: 1,
    })?;
    let deadline = Instant::now() + WAIT;
    let session = loop {
        match reply.try_recv() {
            Ok(result) => break result?,
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(1))
            }
            _ => {
                return Err("measured start acknowledgement unavailable before deadline".into());
            }
        }
    };
    row.session = Some(session);
    let source = runtime.take_input(WAIT)?;
    Ok((session, source))
}
fn observe_terminal(
    runtime: &MeasuredRuntime,
    row: &mut Observation,
    t0: Instant,
    stopped: Instant,
    frames: usize,
    expected: ExpectedOutcome,
) -> Result<String, Box<dyn std::error::Error>> {
    let session = row.session.ok_or("missing current measurement session")?;
    let deadline = Instant::now() + WAIT;
    let timing = loop {
        let timing = runtime.timing()?;
        if timing.session != Some(session) {
            return Err("measurement session identity changed".into());
        }
        offsets(row, timing, t0);
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
    if row.stop_to_terminal_nanos.is_none() {
        return Err("accepted terminal timestamp precedes stop or is missing".into());
    }
    validate_engine_timing(timing.decode_timing, frames, expected)?;
    if expected == ExpectedOutcome::Empty {
        if timing.terminal != Some(MeasuredTerminal::Empty) || timing.t_text.is_some() {
            return Err("run did not end with explicitly expected Empty outcome".into());
        }
        if runtime.snapshot()?.session.text.is_some() {
            return Err("Empty outcome retained unexpected session text".into());
        }
        return Ok(String::new());
    }
    if timing.terminal != Some(MeasuredTerminal::Local) || timing.t_text.is_none() {
        return Err("run ended without accepted nonempty final controller text".into());
    }
    let state = runtime.snapshot()?;
    state
        .session
        .text
        .ok_or_else(|| "controller final text is missing".into())
}
fn finish_result(
    runtime: &MeasuredRuntime,
    mut row: Observation,
    result: Result<String, Box<dyn std::error::Error>>,
    expectation: Expectation<'_>,
) -> Output {
    match result {
        Ok(text) => {
            row.reference_match = expectation
                .reference
                .map(|expected| text.trim() == expected.trim());
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
            } else if expectation.outcome == ExpectedOutcome::Local
                && row.stop_to_text_nanos.is_none()
            {
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
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: None,
            },
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
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: None,
            },
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
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: None,
            },
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
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: Some("controlled words"),
                context: None,
            },
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
    #[test]
    fn explicitly_expected_empty_completes_without_a_text_latency()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Empty;
        impl Decoder for Empty {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                Ok(String::new())
            }
        }
        let measured = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Empty)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_ready(&measured)?;
        let output = one_scoped(
            &measured,
            &[0.125; 320],
            Language::German,
            "expected-empty".into(),
            false,
            Expectation {
                outcome: ExpectedOutcome::Empty,
                reference: None,
                context: None,
            },
            || {
                Ok(FakeScope {
                    ends: std::rc::Rc::new(std::cell::Cell::new(0)),
                    failure: None,
                })
            },
        );
        let ended = cleanup(measured);
        assert!(ended.joined);
        assert_eq!(output.text.as_deref(), Some(""));
        assert_eq!(output.observation.terminal.as_deref(), Some("Empty"));
        assert_eq!(output.observation.stop_to_text_nanos, None);
        assert_eq!(output.observation.t_text_offset_nanos, None);
        assert!(output.observation.t_terminal_offset_nanos.is_some());
        assert!(output.observation.stop_to_terminal_nanos.is_some());
        assert_eq!(output.observation.engine_input_frames, Some(320));
        assert!(output.observation.engine_elapsed_nanos.is_some());
        assert!(output.observation.engine_rtf.is_some());
        assert!(matches!(output.observation.status, "ok" | "invalid_setup"));
        Ok(())
    }

    #[test]
    fn missing_or_wrong_frame_engine_measurement_cannot_qualify_local() {
        use everlast_dictation::inference_service::DecodeTiming;
        assert!(validate_engine_timing(None, 320, ExpectedOutcome::Local).is_err());
        assert!(validate_engine_timing(None, 320, ExpectedOutcome::Empty).is_ok());
        let measured = Some(DecodeTiming {
            elapsed: Duration::from_millis(10),
            input_frames: 319,
        });
        assert!(validate_engine_timing(measured, 320, ExpectedOutcome::Local).is_err());
        assert!(validate_engine_timing(measured, 320, ExpectedOutcome::Empty).is_err());
    }

    #[test]
    fn default_local_rejects_an_empty_result() -> Result<(), Box<dyn std::error::Error>> {
        struct Empty;
        impl Decoder for Empty {
            fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
                Ok(String::new())
            }
        }
        let measured = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Empty)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_ready(&measured)?;
        let output = one_scoped(
            &measured,
            &[0.125; 320],
            Language::German,
            "unexpected-empty".into(),
            false,
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: None,
            },
            || {
                Ok(FakeScope {
                    ends: std::rc::Rc::new(std::cell::Cell::new(0)),
                    failure: None,
                })
            },
        );
        assert!(cleanup(measured).joined);
        assert_eq!(output.observation.status, "failed");
        assert_eq!(output.observation.terminal.as_deref(), Some("Empty"));
        assert_eq!(output.text, None);
        assert_eq!(output.observation.stop_to_text_nanos, None);
        assert!(
            output
                .observation
                .error
                .as_deref()
                .is_some_and(|value| value.contains("nonempty"))
        );
        Ok(())
    }
}
