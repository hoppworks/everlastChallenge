//! Owned publisher scheduling for explicit developer measurement only.
use super::*;
use crossbeam_channel::{Receiver, Sender, bounded};
use everlast_dictation::{
    audio::replay::{
        ReplayInput,
        clock::{Publication, feed_observed},
    },
    runtime::measurement::MeasuredStopHandle,
};
use macos_integration::realtime::{
    self, PolicyError, PolicySnapshot, PublisherPolicy, PublisherReport, TimeConstraint,
};
use std::{sync::Arc, thread::JoinHandle};

pub(super) struct Input {
    pub source: ReplayInput,
    pub stop: MeasuredStopHandle,
}
#[derive(Default)]
pub(super) struct Published {
    pub t0: Option<Instant>,
    pub stopped: Option<Instant>,
    pub feed: Option<FeedEvidence>,
    pub joined: bool,
    pub policy: Option<serde_json::Value>,
    pub errors: Vec<String>,
}
pub(super) trait Publisher {
    fn ready(&mut self) -> Result<(), String>;
    fn submit(&mut self, input: Input) -> Result<(), String>;
    fn close(&mut self);
    fn join(&mut self, budget: Duration) -> Published;
}
pub(super) fn orchestrate(
    publisher: &mut impl Publisher,
    prepare: impl FnOnce() -> Result<Input, String>,
    budget: Duration,
) -> Published {
    let input = publisher
        .ready()
        .and_then(|()| prepare())
        .and_then(|input| publisher.submit(input));
    publisher.close();
    let mut result = publisher.join(budget);
    if let Err(error) = input {
        result.errors.push(error);
    }
    result
}

struct Outlier {
    end_frame: usize,
    after_publish_nanos: u128,
    read: Result<PolicySnapshot, PolicyError>,
}
#[derive(Default)]
struct Payload {
    t0: Option<Instant>,
    stopped: Option<Instant>,
    feed: Option<FeedEvidence>,
    before: Option<Result<PolicySnapshot, PolicyError>>,
    after: Option<Result<PolicySnapshot, PolicyError>>,
    outliers: Vec<Outlier>,
    errors: Vec<String>,
}
fn publish(
    input: Input,
    samples: &[f32],
    mut outliers: Vec<Outlier>,
    mut read: impl FnMut() -> Result<PolicySnapshot, PolicyError>,
) -> Payload {
    let Input { mut source, stop } = input;
    let mut output = Payload {
        t0: Some(source.started),
        before: Some(read()),
        ..Payload::default()
    };
    match feed_observed(&mut source, samples, |publication: &Publication| {
        outliers.push(Outlier {
            end_frame: publication.end_frame,
            after_publish_nanos: publication.after_publish_nanos,
            read: read(),
        });
    }) {
        Ok(feed) => {
            output.feed = Some(feed);
            match stop.stop() {
                Ok(at) => output.stopped = Some(at),
                Err(error) => output.errors.push(error.to_string()),
            }
        }
        Err(error) => output.errors.push(error.to_string()),
    }
    // Stop ingress precedes port release and every after-feed policy query.
    drop(source.port);
    output.after = Some(read());
    output.outliers = outliers;
    output
}

pub(super) struct Native {
    ready: Receiver<()>,
    done: Receiver<()>,
    send: Option<Sender<Input>>,
    child: Option<JoinHandle<PublisherReport<Payload>>>,
}
impl Native {
    pub fn spawn(samples: Arc<Vec<f32>>) -> Result<Self, String> {
        let mut outliers = Vec::new();
        let count = samples.len().div_ceil(320);
        if samples.is_empty() || samples.len() > 4_800_000 {
            return Err("scheduled fixture outside 1..4800000 mono frames".into());
        }
        outliers
            .try_reserve_exact(count)
            .map_err(|error| error.to_string())?;
        let (ack, ready) = bounded(1);
        let (send, receive) = bounded::<Input>(1);
        let (finished, done) = bounded(1);
        let child = realtime::spawn_publisher(PublisherPolicy::TimeConstrained, move |observer| {
            if ack.try_send(()).is_err() {
                return Payload {
                    errors: vec!["publisher readiness receiver closed".into()],
                    ..Payload::default()
                };
            }
            let output = match receive.recv_timeout(WAIT) {
                Ok(input) => publish(input, &samples, outliers, || observer.snapshot()),
                Err(error) => Payload {
                    errors: vec![format!("publisher input unavailable: {error}")],
                    ..Payload::default()
                },
            };
            if finished.try_send(()).is_err() {};
            output
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            ready,
            done,
            send: Some(send),
            child: Some(child),
        })
    }
}
impl Publisher for Native {
    fn ready(&mut self) -> Result<(), String> {
        self.ready
            .recv_timeout(WAIT)
            .map_err(|error| format!("publisher readiness failed: {error}"))
    }
    fn submit(&mut self, input: Input) -> Result<(), String> {
        self.send
            .as_ref()
            .ok_or("publisher input channel closed")?
            .try_send(input)
            .map_err(|error| format!("publisher input failed: {error}"))
    }
    fn close(&mut self) {
        self.send.take();
    }
    fn join(&mut self, budget: Duration) -> Published {
        let Some(child) = self.child.take() else {
            return Published {
                errors: vec!["publisher join handle missing".into()],
                ..Published::default()
            };
        };
        let Some(deadline) = Instant::now().checked_add(budget) else {
            return Published {
                errors: vec!["publisher join clock overflow; join unverified".into()],
                ..Published::default()
            };
        };
        if matches!(
            self.done
                .recv_timeout(deadline.saturating_duration_since(Instant::now())),
            Err(crossbeam_channel::RecvTimeoutError::Timeout)
        ) {
            self.child = Some(child);
            return Published {
                errors: vec![
                    "publisher completion timed out; join and child output unverified".into(),
                ],
                ..Published::default()
            };
        }
        // Completion (or sender drop on setup failure) ends the long wait.
        // Only final native readback/thread exit is polled, using the SAME deadline.
        while !child.is_finished() {
            if Instant::now() >= deadline {
                self.child = Some(child);
                return Published {
                    errors: vec![
                        "publisher exit timed out; join and child output unverified".into(),
                    ],
                    ..Published::default()
                };
            }
            thread::sleep(Duration::from_millis(1));
        }
        match child.join() {
            Ok(report) => collect_report(report),
            Err(_) => Published {
                errors: vec!["publisher panicked; join failed".into()],
                ..Published::default()
            },
        }
    }
}
fn constraint_value(tc: TimeConstraint) -> serde_json::Value {
    serde_json::json!({"periodTicks":tc.period_ticks,"computationTicks":tc.computation_ticks,
        "constraintTicks":tc.constraint_ticks,"preemptible":tc.preemptible})
}
fn snapshot_value(snapshot: PolicySnapshot) -> serde_json::Value {
    serde_json::json!({"timeConstraint":constraint_value(snapshot.time_constraint),"defaultPolicy":snapshot.default_policy,
        "basicPolicy":snapshot.basic_policy,"cpu":{"userNanos":snapshot.cpu.user.as_nanos(),"systemNanos":snapshot.cpu.system.as_nanos()}})
}
fn validate(
    snapshot: PolicySnapshot,
    expected: Option<TimeConstraint>,
    point: &str,
    errors: &mut Vec<String>,
) {
    if let Some(expected) = expected
        && (snapshot.default_policy
            || snapshot.basic_policy != 2
            || snapshot.time_constraint != expected)
    {
        errors.push(format!(
            "{point}: time-constraint readback differs (defaultPolicy={}, basicPolicy={})",
            snapshot.default_policy, snapshot.basic_policy
        ));
    }
}
fn read_value(
    read: Result<PolicySnapshot, PolicyError>,
    expected: Option<TimeConstraint>,
    point: &str,
    errors: &mut Vec<String>,
) -> serde_json::Value {
    match read {
        Ok(snapshot) => {
            validate(snapshot, expected, point, errors);
            serde_json::json!({"snapshot":snapshot_value(snapshot),"error":null})
        }
        Err(error) => {
            let error = error.to_string();
            errors.push(format!("{point}: {error}"));
            serde_json::json!({"snapshot":null,"error":error})
        }
    }
}
fn collect_report(report: PublisherReport<Payload>) -> Published {
    let expected = report.requested;
    let mut result = Published {
        joined: true,
        ..Published::default()
    };
    if report.policy != PublisherPolicy::TimeConstrained {
        result
            .errors
            .push("native publisher returned an unexpected policy".into());
    }
    if expected.is_none() {
        result
            .errors
            .push("native publisher omitted requested time-constraint ticks".into());
    }
    for (point, present) in [
        ("before", report.before.is_some()),
        ("configured", report.configured.is_some()),
        ("post", report.after.is_some()),
    ] {
        if !present {
            result
                .errors
                .push(format!("{point}: policy snapshot missing"));
        }
    }
    for error in [report.setup_error, report.post_error, report.finish_error]
        .into_iter()
        .flatten()
    {
        result.errors.push(error.to_string());
    }
    for (point, snapshot) in [("configured", report.configured), ("post", report.after)] {
        if let Some(snapshot) = snapshot {
            validate(snapshot, expected, point, &mut result.errors);
        }
    }
    let mut before_feed = None;
    let mut after_feed = None;
    let mut outliers = Vec::new();
    if let Some(output) = report.output {
        if output.before.is_none() {
            result
                .errors
                .push("beforeFeed: policy snapshot missing".into());
        }
        if output.after.is_none() {
            result
                .errors
                .push("afterFeed: policy snapshot missing".into());
        }
        result.t0 = output.t0;
        result.stopped = output.stopped;
        result.feed = output.feed;
        result.errors.extend(output.errors);
        before_feed = output
            .before
            .map(|read| read_value(read, expected, "beforeFeed", &mut result.errors));
        after_feed = output
            .after
            .map(|read| read_value(read, expected, "afterFeed", &mut result.errors));
        outliers=output.outliers.into_iter().map(|row|serde_json::json!({"endFrame":row.end_frame,"afterPublishNanos":row.after_publish_nanos,
            "policy":read_value(row.read,expected,"invalidPublication",&mut result.errors)})).collect();
    }
    result.policy = Some(
        serde_json::json!({"policy":match report.policy {PublisherPolicy::TimeConstrained=>"timeConstrained",PublisherPolicy::UserInteractive=>"userInteractive"},"requested":expected.map(constraint_value),
        "before":report.before.map(snapshot_value),"configured":report.configured.map(snapshot_value),"after":report.after.map(snapshot_value),
        "setupError":report.setup_error.map(|error|error.to_string()),"postError":report.post_error.map(|error|error.to_string()),
        "finishError":report.finish_error.map(|error|error.to_string()),"beforeFeed":before_feed,"afterFeed":after_feed,"outliers":outliers}),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use dictation_core::audio::AudioFormat;
    use everlast_dictation::{
        inference_service::{Decoder, InferenceService},
        transcription::InferenceError,
    };
    use std::sync::{Arc, Mutex};
    struct Words;
    impl Decoder for Words {
        fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
            if audio.len() != 320 || audio.iter().any(|value| *value != 0.125) {
                return Err(InferenceError::InvalidAudio);
            }
            Ok("controlled words".into())
        }
    }
    fn runtime() -> Result<MeasuredRuntime, Box<dyn std::error::Error>> {
        let runtime = MeasuredRuntime::spawn(
            InferenceService::with_decoder(Words)?,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_ready(&runtime)?;
        Ok(runtime)
    }
    struct Fake {
        events: Arc<Mutex<Vec<&'static str>>>,
    }
    impl Fake {
        fn event(&self, event: &'static str) {
            self.events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(event);
        }
    }
    impl Publisher for Fake {
        fn ready(&mut self) -> Result<(), String> {
            self.event("ready");
            Ok(())
        }
        fn submit(&mut self, _: Input) -> Result<(), String> {
            self.event("submit");
            Ok(())
        }
        fn close(&mut self) {
            self.event("close");
        }
        fn join(&mut self, _: Duration) -> Published {
            self.event("join");
            Published {
                joined: true,
                ..Published::default()
            }
        }
    }
    #[test]
    fn ready_precedes_real_runtime_start_and_join_follows_port_handoff()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = runtime()?;
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut fake = Fake {
            events: Arc::clone(&events),
        };
        let mut row = observation("owned".into(), false);
        let report = orchestrate(
            &mut fake,
            || {
                events.lock().map_err(|_| "events poisoned")?.push("start");
                let (session, source) =
                    start_input(&runtime, Language::German, &mut row).map_err(|e| e.to_string())?;
                Ok(Input {
                    source,
                    stop: runtime.stop_handle(session),
                })
            },
            Duration::from_secs(2),
        );
        assert!(report.joined);
        let ended = cleanup(runtime);
        assert!(ended.joined);
        assert!(row.session.is_some());
        assert_eq!(
            *events.lock().map_err(|_| "events poisoned")?,
            ["ready", "start", "submit", "close", "join"]
        );
        Ok(())
    }
    fn valid_snapshot() -> PolicySnapshot {
        PolicySnapshot {
            time_constraint: TimeConstraint {
                period_ticks: 20_000_000,
                computation_ticks: 500_000,
                constraint_ticks: 1_000_000,
                preemptible: true,
            },
            default_policy: false,
            basic_policy: 2,
            cpu: realtime::ThreadCpuTime {
                user: Duration::ZERO,
                system: Duration::ZERO,
            },
        }
    }
    fn fake_thread(samples: Arc<Vec<f32>>, setup_error: bool, post_error: bool) -> Native {
        let (ack, ready) = bounded(1);
        let (send, receive) = bounded::<Input>(1);
        let (finished, done) = bounded(1);
        let child = thread::spawn(move || {
            let snapshot = valid_snapshot();
            let mut report = PublisherReport {
                policy: PublisherPolicy::TimeConstrained,
                output: None,
                requested: Some(snapshot.time_constraint),
                before: Some(snapshot),
                configured: Some(snapshot),
                after: Some(snapshot),
                setup_error: None,
                post_error: None,
                finish_error: None,
            };
            if setup_error {
                drop(ack);
                report.setup_error = Some(PolicyError::Os {
                    operation: "fake setup",
                    code: 17,
                });
                return report;
            }
            if ack.try_send(()).is_err() {
                return report;
            }
            if let Ok(input) = receive.recv_timeout(Duration::from_secs(2)) {
                report.output = Some(publish(
                    input,
                    &samples,
                    Vec::with_capacity(samples.len().div_ceil(320)),
                    || Ok(snapshot),
                ));
            }
            if post_error {
                report.after = None;
                report.post_error = Some(PolicyError::Os {
                    operation: "fake post",
                    code: 18,
                });
            }
            if finished.try_send(()).is_err() {};
            report
        });
        Native {
            ready,
            done,
            send: Some(send),
            child: Some(child),
        }
    }
    #[test]
    fn fake_owned_child_runs_real_feed_stop_and_common_terminal_checks_without_rt_set()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = runtime()?;
        let samples = Arc::new(vec![0.125; 320]);
        let child_samples = Arc::clone(&samples);
        let output = one_scheduled_with(
            &runtime,
            &samples,
            Language::German,
            "fake-child".into(),
            false,
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: Some("controlled words"),
                context: None,
            },
            || Ok(fake_thread(child_samples, false, false)),
        );
        assert!(cleanup(runtime).joined);
        assert_eq!(output.observation.status, "ok");
        assert_eq!(output.observation.publisher_joined, Some(true));
        assert!(output.observation.controller_clock_matches_capture);
        assert!(output.observation.stop_to_text_nanos.is_some());
        assert_eq!(output.observation.engine_input_frames, Some(320));
        assert!(output.observation.engine_rtf.is_some());
        assert_eq!(samples.len(), 320);
        assert_eq!(
            Arc::strong_count(&samples),
            1,
            "publisher retained fixture ownership after join"
        );
        Ok(())
    }
    #[test]
    fn actual_child_setup_failure_never_starts_a_runtime_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = runtime()?;
        let samples = Arc::new(vec![0.125; 320]);
        let child_samples = Arc::clone(&samples);
        let output = one_scheduled_with(
            &runtime,
            &samples,
            Language::German,
            "setup-fail".into(),
            false,
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: None,
            },
            || Ok(fake_thread(child_samples, true, false)),
        );
        assert_eq!(runtime.timing()?.session, None);
        assert!(cleanup(runtime).joined);
        assert_eq!(output.observation.status, "invalid_setup");
        assert_eq!(output.observation.session, None);
        assert_eq!(output.observation.publisher_joined, Some(true));
        assert!(
            output
                .observation
                .error
                .as_deref()
                .is_some_and(|error| error.contains("fake setup"))
        );
        Ok(())
    }
    #[test]
    fn child_post_error_preserves_feed_stop_text_and_engine_evidence_but_invalidates_run()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = runtime()?;
        let samples = Arc::new(vec![0.125; 320]);
        let child_samples = Arc::clone(&samples);
        let output = one_scheduled_with(
            &runtime,
            &samples,
            Language::German,
            "post-fail".into(),
            false,
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: None,
            },
            || Ok(fake_thread(child_samples, false, true)),
        );
        assert!(cleanup(runtime).joined);
        assert_eq!(output.observation.status, "invalid_setup");
        assert!(output.observation.feed.is_some());
        assert!(output.observation.t_stop_offset_nanos.is_some());
        assert!(output.observation.t_text_offset_nanos.is_some());
        assert_eq!(output.observation.engine_input_frames, Some(320));
        assert_eq!(output.text.as_deref(), Some("controlled words"));
        assert!(
            output
                .observation
                .error
                .as_deref()
                .is_some_and(|error| error.contains("fake post"))
        );
        Ok(())
    }
    #[test]
    fn unverified_publisher_join_is_bounded_and_does_not_hold_runtime_owner()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = runtime()?;
        let (ack, ready) = bounded(1);
        let (send, receive) = bounded::<Input>(1);
        let (held, entered) = bounded(1);
        let (release, resume) = bounded(1);
        let (finished, done) = bounded(1);
        let child = thread::spawn(move || {
            if ack.send(()).is_err() {
                return PublisherReport {
                    policy: PublisherPolicy::TimeConstrained,
                    output: None,
                    requested: None,
                    before: None,
                    configured: None,
                    after: None,
                    setup_error: None,
                    post_error: None,
                    finish_error: None,
                };
            }
            let input = receive.recv_timeout(Duration::from_secs(2)).ok();
            if held.send(()).is_err() {};
            if resume.recv_timeout(Duration::from_secs(2)).is_err() {};
            drop(input);
            if finished.try_send(()).is_err() {};
            PublisherReport {
                policy: PublisherPolicy::TimeConstrained,
                output: None,
                requested: None,
                before: None,
                configured: None,
                after: None,
                setup_error: None,
                post_error: None,
                finish_error: None,
            }
        });
        let mut publisher = Native {
            ready,
            done,
            send: Some(send),
            child: Some(child),
        };
        publisher.ready()?;
        let mut row = observation("held-child".into(), false);
        let (session, source) = start_input(&runtime, Language::German, &mut row)?;
        publisher.submit(Input {
            source,
            stop: runtime.stop_handle(session),
        })?;
        publisher.close();
        entered.recv_timeout(Duration::from_secs(2))?;
        let report = publisher.join(Duration::ZERO);
        assert!(!report.joined);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("unverified"))
        );
        let cleaned = cleanup(runtime);
        // Release the controlled parked child before assertions/unwinding.
        if release.try_send(()).is_err() {};
        let joined = publisher.join(Duration::from_secs(2));
        assert!(
            cleaned.joined,
            "held StopHandle prevented consuming Runtime cleanup"
        );
        assert!(joined.joined);
        Ok(())
    }

    #[test]
    fn missing_requested_or_unexpected_native_policy_cannot_become_valid_tc_evidence() {
        for (policy, requested) in [
            (PublisherPolicy::TimeConstrained, None),
            (
                PublisherPolicy::UserInteractive,
                Some(valid_snapshot().time_constraint),
            ),
        ] {
            let snapshot = valid_snapshot();
            let report = PublisherReport {
                policy,
                requested,
                before: Some(snapshot),
                configured: Some(snapshot),
                after: Some(snapshot),
                output: Some(Payload::default()),
                setup_error: None,
                post_error: None,
                finish_error: None,
            };
            let result = collect_report(report);
            assert!(
                !result.errors.is_empty(),
                "unverifiable native admission was accepted"
            );
        }
    }
    #[test]
    fn all_post_setup_tc_readbacks_are_checked_including_invalid_publication_observer() {
        let valid = valid_snapshot();
        let bad = PolicySnapshot {
            basic_policy: 1,
            ..valid
        };
        for point in [
            "configured",
            "post",
            "beforeFeed",
            "afterFeed",
            "invalidPublication",
        ] {
            let payload = Payload {
                before: Some(Ok(if point == "beforeFeed" { bad } else { valid })),
                after: Some(Ok(if point == "afterFeed" { bad } else { valid })),
                outliers: vec![Outlier {
                    end_frame: 320,
                    after_publish_nanos: 31_000_000,
                    read: Ok(if point == "invalidPublication" {
                        bad
                    } else {
                        valid
                    }),
                }],
                ..Payload::default()
            };
            let report = PublisherReport {
                policy: PublisherPolicy::TimeConstrained,
                requested: Some(valid.time_constraint),
                before: Some(bad),
                configured: Some(if point == "configured" { bad } else { valid }),
                after: Some(if point == "post" { bad } else { valid }),
                output: Some(payload),
                setup_error: None,
                post_error: None,
                finish_error: None,
            };
            let result = collect_report(report);
            assert!(
                result.errors.iter().any(|error| error.starts_with(point)),
                "a required TC observation escaped validation"
            );
        }
    }
    #[test]
    fn context_receipt_is_bound_after_start_ack_before_real_capture_decode()
    -> Result<(), Box<dyn std::error::Error>> {
        let (service, receipts) = super::super::audio_context::controlled_service(
            super::super::audio_context::Variant::Reduced,
        )?;
        let runtime = MeasuredRuntime::spawn(
            service,
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
        )?;
        wait_ready(&runtime)?;
        let samples = Arc::new(vec![0.125; 320]);
        let output = one_scheduled_with(
            &runtime,
            &samples,
            Language::German,
            "context-job".into(),
            false,
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: Some(&receipts),
            },
            || Ok(fake_thread(Arc::clone(&samples), false, false)),
        );
        let joined = cleanup(runtime);
        assert!(joined.joined);
        assert_eq!(output.observation.status, "ok");
        let evidence = output.observation.audio_context.ok_or("missing receipt")?;
        assert!(evidence["error"].is_null());
        assert_eq!(
            evidence["receipt"]["identity"]["session"].as_u64(),
            output.observation.session
        );
        assert_eq!(evidence["receipt"]["identity"]["endFrame"], 320);
        assert_eq!(
            evidence["receipt"]["observation"]["audioCtxParameter"],
            1024
        );
        assert!(receipts.finalize().error.is_none());
        Ok(())
    }

    #[test]
    fn missing_context_receipt_invalidates_real_runtime_output_and_next_admission()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = runtime()?;
        let receipts = super::super::audio_context::Receipts::default();
        let samples = Arc::new(vec![0.125; 320]);
        let output = one_scheduled_with(
            &runtime,
            &samples,
            Language::German,
            "missing-context".into(),
            false,
            Expectation {
                outcome: ExpectedOutcome::Local,
                reference: None,
                context: Some(&receipts),
            },
            || Ok(fake_thread(Arc::clone(&samples), false, false)),
        );
        assert!(cleanup(runtime).joined);
        assert_eq!(output.observation.status, "failed");
        assert!(
            output
                .observation
                .audio_context
                .ok_or("missing error evidence")?["error"]
                .is_string()
        );
        assert!(receipts.arm(2, 1, 2, 320, Language::German).is_err());
        Ok(())
    }
}
