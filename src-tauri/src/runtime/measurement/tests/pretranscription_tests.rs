use super::*;
use crossbeam_channel::{Receiver, Sender};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct HeldDecoder {
    entered: Sender<Vec<f32>>,
    release: Receiver<()>,
    calls: usize,
}
impl Decoder for HeldDecoder {
    fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
        self.calls += 1;
        self.entered
            .send(audio.to_vec())
            .map_err(|_| InferenceError::Decode)?;
        if self.calls == 1 {
            self.release
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| InferenceError::Decode)?;
        }
        Ok(if self.calls == 1 { "first" } else { "second" }.into())
    }
}

fn feed(port: &mut crate::audio::CapturePort, samples: &[f32]) {
    // This test feeds logical audio, not a real-time latency measurement. Small
    // batches let the real bounded capture reader drain without an overflow.
    for chunk in samples.chunks(320) {
        port.publish(chunk);
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn confirmed_pause_decodes_during_recording_and_final_rest_covers_every_frame() -> TestResult {
    let (entered, entries) = bounded(2);
    let (release, gate) = bounded(1);
    let runtime = MeasuredRuntime::spawn_pretranscription(
        InferenceService::with_decoder(HeldDecoder {
            entered,
            release: gate,
            calls: 0,
        })?,
        1,
        AudioFormat {
            sample_rate: 16000,
            channels: 1,
        },
    )?;
    guarded(runtime, release.clone(), |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let session = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut original = vec![0.1f32; 16000];
        original.extend_from_slice(&[0.0; 8000]);
        feed(&mut input.port, &original);
        let first = entries
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "confirmed pause did not start a decoder before Stop")?;
        let early_trace = runtime.job_trace()?;
        assert!(
            early_trace.enabled,
            "candidate must retain a bounded actual-job trace"
        );
        assert_eq!(early_trace.jobs.len(), 1);
        let early = &early_trace.jobs[0];
        assert_eq!((early.timeline_start, early.timeline_end), (0, 19200));
        assert_eq!((early.storage_start, early.storage_end), (0, 19200));
        assert!(early.host_submit_offset_nanos.is_some());
        assert!(early.worker_before_decode_offset_nanos.is_some());
        assert_eq!(early.worker_after_decode_offset_nanos, None);
        assert_eq!(early.result_adopted_offset_nanos, None);
        assert_eq!(first.len(), 19200);
        assert_eq!(first, original[..19200]);
        assert_eq!(runtime.snapshot()?.session.phase, Phase::Recording);
        assert_eq!(runtime.timing()?.t_text, None);
        let mut tail = vec![0.2f32; 8000];
        tail.extend_from_slice(&[0.3125; 137]);
        feed(&mut input.port, &tail);
        original.extend_from_slice(&tail);
        runtime.stop(session)?;
        wait_state(runtime, |s| s.session.phase == Phase::Finalizing)?;
        assert_eq!(runtime.snapshot()?.session.text, None);
        assert_eq!(runtime.timing()?.t_text, None);
        release.try_send(())?;
        let rest = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(rest.last(), Some(&0.3125));
        let mut decoded = first;
        decoded.extend(rest);
        assert_eq!(
            decoded, original,
            "decoder ranges must neither overlap nor omit PCM"
        );
        let final_state = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(final_state.session.text.as_deref(), Some("first second"));
        assert_eq!(final_state.session.outcome, Some(Outcome::Local));
        assert!(runtime.timing()?.t_text.is_some());
        assert!(entries.is_empty());
        let trace = runtime.job_trace()?;
        assert_eq!(trace.jobs.len(), 2);
        assert_eq!(trace.error, None);
        let rest = &trace.jobs[1];
        assert_eq!(
            (rest.timeline_start, rest.timeline_end),
            (19200, original.len())
        );
        for job in &trace.jobs {
            assert_eq!(job.session, session);
            assert_eq!(job.generation, 1);
            assert_eq!(job.worker_outcome, Some("decoded-ok"));
            assert_eq!(job.adoption, Some("accepted"));
            assert!(job.host_submit_offset_nanos <= job.service_accepted_offset_nanos);
            assert!(job.service_accepted_offset_nanos <= job.worker_before_decode_offset_nanos);
            assert!(job.worker_before_decode_offset_nanos <= job.worker_after_decode_offset_nanos);
            assert!(job.worker_after_decode_offset_nanos <= job.result_adopted_offset_nanos);
        }
        Ok(())
    })
}

fn guarded(
    runtime: MeasuredRuntime,
    release: Sender<()>,
    check: impl FnOnce(&MeasuredRuntime) -> TestResult,
) -> TestResult {
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&runtime)));
    let _released = release.try_send(());
    let shutdown = runtime.shutdown();
    let joined = runtime.join();
    match checked {
        Err(payload) => {
            shutdown?;
            joined?;
            std::panic::resume_unwind(payload);
        }
        Ok(result) => {
            result?;
            shutdown?;
            joined?;
            Ok(())
        }
    }
}

struct HeldFixture {
    runtime: MeasuredRuntime,
    entries: Receiver<Vec<f32>>,
    release: Sender<()>,
}
fn held_runtime() -> Result<HeldFixture, Box<dyn std::error::Error>> {
    let (entered, entries) = bounded(4);
    let (release, gate) = bounded(1);
    let runtime = MeasuredRuntime::spawn_pretranscription(
        InferenceService::with_decoder(HeldDecoder {
            entered,
            release: gate,
            calls: 0,
        })?,
        1,
        AudioFormat {
            sample_rate: 16000,
            channels: 1,
        },
    )?;
    Ok(HeldFixture {
        runtime,
        entries,
        release,
    })
}

#[test]
fn multiple_pauses_while_decoder_is_held_keep_only_latest_cut_then_ordered_final_text() -> TestResult
{
    let HeldFixture {
        runtime,
        entries,
        release,
    } = held_runtime()?;
    guarded(runtime, release.clone(), |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let sid = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut all = vec![0.1; 16000];
        all.resize(24000, 0.0);
        feed(&mut input.port, &all);
        let first = entries.recv_timeout(Duration::from_secs(2))?;
        let mut backlog = vec![0.2; 8000];
        backlog.resize(16000, 0.0);
        backlog.extend_from_slice(&[0.3; 8000]);
        backlog.resize(48000, 0.0);
        feed(&mut input.port, &backlog);
        all.extend(backlog);
        assert!(
            entries.recv_timeout(Duration::from_millis(60)).is_err(),
            "backlog must not enqueue more PCM jobs"
        );
        assert_eq!(runtime.snapshot()?.session.phase, Phase::Recording);
        release.send(())?;
        let second = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(
            second.len(),
            51200 - 19200,
            "latest confirmed cut coalesces both later speech regions"
        );
        assert!(
            entries.recv_timeout(Duration::from_millis(60)).is_err(),
            "prolonged silence must not produce empty jobs"
        );
        assert_eq!(runtime.timing()?.t_text, None);
        let end = [0.4; 137];
        feed(&mut input.port, &end);
        all.extend(end);
        runtime.stop(sid)?;
        let rest = entries.recv_timeout(Duration::from_secs(2))?;
        let raw: Vec<f32> = first.into_iter().chain(second).chain(rest).collect();
        assert_eq!(raw, all);
        let finished = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(
            finished.session.text.as_deref(),
            Some("first second second")
        );
        assert_eq!(
            runtime
                .timing()?
                .decode_timing
                .ok_or("missing accumulated timing")?
                .input_frames,
            all.len()
        );
        Ok(())
    })
}

#[test]
fn cancelled_old_decoder_cannot_seal_new_recording_or_publish_its_text() -> TestResult {
    let HeldFixture {
        runtime,
        entries,
        release,
    } = held_runtime()?;
    let reader = runtime.job_trace_handle();
    guarded(runtime, release.clone(), |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let first = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut voice = vec![0.1; 16000];
        voice.resize(24000, 0.0);
        feed(&mut input.port, &voice);
        entries.recv_timeout(Duration::from_secs(2))?;
        runtime.cancel(first)?;
        wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        // Normal public Client admission deliberately permits a new recording
        // while one revoked native call finishes. Measured starts keep their
        // stricter owner-idle handshake; this is a lifecycle, not L measurement.
        await_value(
            runtime
                .runtime
                .client
                .start(settings(), None, false, false)?,
        )?
        .map_err(std::io::Error::other)?;
        let second = wait_state(runtime, |s| s.session.phase == Phase::Recording)?
            .session
            .session
            .ok_or("missing new SID")?;
        assert!(second > first);
        let current = runtime.job_trace()?;
        assert_eq!(current.session, Some(second));
        assert!(current.jobs.is_empty());
        let retired = current
            .retired_job
            .ok_or("old active job evidence not retired")?;
        assert_eq!(retired.session, first);
        assert!(retired.worker_before_decode_offset_nanos.is_some());
        assert_eq!(retired.worker_after_decode_offset_nanos, None);
        let mut next = runtime.input.recv_timeout(Duration::from_secs(2))?;
        feed(&mut next.port, &voice);
        assert!(
            entries.recv_timeout(Duration::from_millis(60)).is_err(),
            "old active PCM plus new journal forbids a third frozen journal"
        );
        assert_eq!(runtime.snapshot()?.session.phase, Phase::Recording);
        release.send(())?;
        let new_prefix = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(new_prefix.len(), 19200);
        runtime.runtime.client.stop(second)?;
        entries.recv_timeout(Duration::from_secs(2))?;
        let ended = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(ended.session.text.as_deref(), Some("second second"));
        assert_eq!(ended.session.outcome, Some(Outcome::Local));
        let current = runtime.job_trace()?;
        assert_eq!(current.error, None);
        assert!(current.jobs.iter().all(|job| job.session == second));
        assert_eq!(
            current
                .retired_job
                .as_ref()
                .and_then(|job| job.worker_outcome),
            Some("cancelled-after-decode")
        );
        Ok(())
    })?;
    let after_join = reader.snapshot()?;
    assert!(
        after_join
            .retired_job
            .as_ref()
            .and_then(|job| job.worker_after_decode_offset_nanos)
            .is_some()
    );
    assert_eq!(after_join.error, None);
    Ok(())
}

#[test]
fn real_replay_resampling_and_partial_end_syllable_match_unsegmented_pcm() -> TestResult {
    let (entered, entries) = bounded(3);
    let (release, gate) = bounded(1);
    let format = AudioFormat {
        sample_rate: 44100,
        channels: 1,
    };
    let runtime = MeasuredRuntime::spawn_pretranscription(
        InferenceService::with_decoder(HeldDecoder {
            entered,
            release: gate,
            calls: 0,
        })?,
        1,
        format,
    )?;
    guarded(runtime, release.clone(), |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let sid = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut source = vec![0.1; 22050];
        source.resize(66150, 0.0);
        feed(&mut input.port, &source);
        let first = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(runtime.snapshot()?.session.phase, Phase::Recording);
        let end = vec![0.3; 10003];
        feed(&mut input.port, &end);
        source.extend(end);
        runtime.stop(sid)?;
        wait_state(runtime, |s| s.session.phase == Phase::Finalizing)?;
        release.send(())?;
        let rest = entries.recv_timeout(Duration::from_secs(2))?;
        let mut reference = dictation_core::audio::AudioProcessor::new(format)?;
        reference.push_interleaved(&source)?;
        let combined: Vec<f32> = first.into_iter().chain(rest).collect();
        assert_eq!(combined, reference.finish()?);
        wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        Ok(())
    })
}

#[test]
fn empty_capture_has_one_terminal_without_a_fake_decoder_timing_or_text_time() -> TestResult {
    let HeldFixture {
        runtime,
        entries,
        release,
    } = held_runtime()?;
    guarded(runtime, release, |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let sid = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let _input = runtime.take_input(Duration::from_secs(2))?;
        wait_state(runtime, |s| s.session.phase == Phase::Recording)?;
        runtime.stop(sid)?;
        let end = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(end.session.outcome, Some(Outcome::Empty));
        assert!(entries.is_empty());
        let time = runtime.timing()?;
        assert_eq!(time.terminal, Some(MeasuredTerminal::Empty));
        assert!(time.t_terminal.is_some());
        assert_eq!(time.t_text, None);
        assert_eq!(time.decode_timing, None);
        Ok(())
    })
}

#[test]
fn three_hundred_seconds_of_logical_uninterrupted_audio_is_one_final_job() -> TestResult {
    struct Count {
        entered: Sender<(usize, bool)>,
    }
    impl Decoder for Count {
        fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
            self.entered
                .send((audio.len(), audio.iter().all(|sample| *sample == 0.1)))
                .map_err(|_| InferenceError::Decode)?;
            Ok("one final job".into())
        }
    }
    let (entered, entries) = bounded(2);
    let (release, _unused) = bounded(1);
    let runtime = MeasuredRuntime::spawn_pretranscription(
        InferenceService::with_decoder(Count { entered })?,
        1,
        AudioFormat {
            sample_rate: 16000,
            channels: 1,
        },
    )?;
    guarded(runtime, release, |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        // Frame-limit exercise accelerated on the real owner, not a wall-clock
        // 300-s trial. No native model, microphone or latency qualification.
        let total = dictation_core::audio::MAX_SAMPLES;
        let mut remaining = total;
        while remaining != 0 {
            let count = remaining.min(1024);
            input.port.publish(&[0.1f32; 1024][..count]);
            remaining -= count;
            if remaining != 0 {
                assert!(entries.is_empty(), "no pause means no early decoder job");
            }
            thread::sleep(Duration::from_millis(2));
        }
        let (frames, exact) = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(frames, total);
        assert!(exact);
        let end = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(end.session.text.as_deref(), Some("one final job"));
        assert!(end.maximum_reached);
        assert!(entries.is_empty());
        assert_eq!(
            runtime
                .timing()?
                .decode_timing
                .ok_or("missing final timing")?
                .input_frames,
            total
        );
        Ok(())
    })
}

#[test]
fn text_budget_rejection_does_not_adopt_rejected_job_timing_or_a_final_text_clock() -> TestResult {
    struct FullText {
        calls: usize,
    }
    impl Decoder for FullText {
        fn decode(&mut self, _: &[f32], _: Language) -> Result<String, InferenceError> {
            self.calls += 1;
            Ok(if self.calls == 1 {
                "a".repeat(256 * 1024)
            } else {
                "b".into()
            })
        }
    }
    let (release, _unused) = bounded(1);
    let runtime = MeasuredRuntime::spawn_pretranscription(
        InferenceService::with_decoder(FullText { calls: 0 })?,
        1,
        AudioFormat {
            sample_rate: 16000,
            channels: 1,
        },
    )?;
    guarded(runtime, release, |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let sid = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut source = vec![0.1; 16000];
        source.resize(24000, 0.0);
        feed(&mut input.port, &source);
        let deadline = Instant::now() + Duration::from_secs(2);
        while runtime.timing()?.decode_timing.is_none() {
            if Instant::now() >= deadline {
                return Err("first result was not adopted".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        runtime.stop(sid)?;
        let final_state = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert!(matches!(
            final_state.session.outcome,
            Some(Outcome::Failed(_))
        ));
        assert_eq!(final_state.session.text, None);
        let timing = runtime.timing()?;
        assert_eq!(timing.t_text, None);
        assert_eq!(timing.t_terminal, None);
        assert_eq!(timing.terminal, Some(MeasuredTerminal::Failed));
        assert_eq!(
            timing
                .decode_timing
                .ok_or("missing earlier accepted job timing")?
                .input_frames,
            19200,
            "rejected final result must not inflate accepted decode coverage"
        );
        Ok(())
    })
}

#[test]
fn bounded_job_trace_overflow_keeps_prior_record_and_rejected_identity() -> TestResult {
    let HeldFixture {
        runtime,
        entries,
        release,
    } = held_runtime()?;
    runtime
        .trace
        .jobs
        .as_ref()
        .ok_or("missing ledger")?
        .lock()
        .map_err(|_| "ledger poisoned")?
        .limit_for_test(1);
    guarded(runtime, release.clone(), |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let sid = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut source = vec![0.1; 16000];
        source.resize(24000, 0.0);
        feed(&mut input.port, &source);
        entries.recv_timeout(Duration::from_secs(2))?;
        runtime.stop(sid)?;
        release.send(())?;
        entries.recv_timeout(Duration::from_secs(2))?;
        wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        let trace = runtime.job_trace()?;
        assert_eq!(trace.jobs.len(), 1);
        assert_eq!(trace.jobs[0].timeline_end, 19200);
        assert!(trace.jobs[0].result_adopted_offset_nanos.is_some());
        assert!(trace.overflow_events > 0);
        assert!(
            trace
                .error
                .as_deref()
                .is_some_and(|error| error.contains("overflow"))
        );
        let rejected = trace
            .first_overflow_job
            .ok_or("overflow identity was lost")?;
        assert_eq!(
            (
                rejected.session,
                rejected.timeline_start,
                rejected.timeline_end
            ),
            (sid, 19200, 24000)
        );
        Ok(())
    })
}

#[test]
fn accepted_old_job_before_decoder_entry_is_retired_and_late_skip_survives_join() -> TestResult {
    struct Entered(Sender<usize>);
    impl Decoder for Entered {
        fn decode(&mut self, audio: &[f32], _: Language) -> Result<String, InferenceError> {
            self.0
                .try_send(audio.len())
                .map_err(|_| InferenceError::Decode)?;
            Ok("new session".into())
        }
    }
    let (entered, entries) = bounded(2);
    let service = InferenceService::with_decoder(Entered(entered))?;
    let (reached, paused) = bounded(1);
    let (release, released) = bounded(1);
    service.pause_before_decode(reached, released)?;
    let runtime = MeasuredRuntime::spawn_pretranscription(
        service,
        1,
        AudioFormat {
            sample_rate: 16000,
            channels: 1,
        },
    )?;
    let reader = runtime.job_trace_handle();
    guarded(runtime, release.clone(), |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let old = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut source = vec![0.1; 16000];
        source.resize(24000, 0.0);
        feed(&mut input.port, &source);
        paused.recv_timeout(Duration::from_secs(2))?;
        let queued = runtime.job_trace()?;
        assert!(queued.jobs[0].service_accepted_offset_nanos.is_some());
        assert_eq!(queued.jobs[0].worker_before_decode_offset_nanos, None);
        runtime.cancel(old)?;
        wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        await_value(
            runtime
                .runtime
                .client
                .start(settings(), None, false, false)?,
        )?
        .map_err(std::io::Error::other)?;
        wait_state(runtime, |s| s.session.phase == Phase::Recording)?;
        let mut new_input = runtime.input.recv_timeout(Duration::from_secs(2))?;
        let retired = runtime
            .job_trace()?
            .retired_job
            .ok_or("accepted/dequeued old identity lost on reset")?;
        assert_eq!(retired.session, old);
        assert_eq!(retired.worker_before_decode_offset_nanos, None);
        release.send(())?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let trace = runtime.job_trace()?;
            if trace
                .retired_job
                .as_ref()
                .and_then(|job| job.worker_settled_offset_nanos)
                .is_some()
            {
                break;
            }
            if Instant::now() >= deadline {
                return Err("late skipped worker evidence missing".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(entries.is_empty(), "cancelled job must not call Decoder");
        feed(&mut new_input.port, &[0.2; 320]);
        let sid = runtime
            .snapshot()?
            .session
            .session
            .ok_or("new SID absent")?;
        runtime.runtime.client.stop(sid)?;
        assert_eq!(entries.recv_timeout(Duration::from_secs(2))?, 320);
        wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        Ok(())
    })?;
    let after = reader.snapshot()?;
    assert_eq!(after.error, None);
    let retired = after
        .retired_job
        .ok_or("retired job missing after owner join")?;
    assert_eq!(retired.worker_outcome, Some("cancelled-before-decode"));
    assert_eq!(retired.worker_before_decode_offset_nanos, None);
    assert_eq!(retired.worker_after_decode_offset_nanos, None);
    assert!(retired.worker_settled_offset_nanos.is_some());
    Ok(())
}
