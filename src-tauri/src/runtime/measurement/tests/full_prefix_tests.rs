//! Actual replay owner + serial service; logical feed, no native model or L claim.
use super::*;
use crossbeam_channel::{Receiver, Sender};
type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Held {
    entered: Sender<Vec<f32>>,
    release: Receiver<()>,
    calls: usize,
}
impl Decoder for Held {
    fn decode(&mut self, samples: &[f32], _: Language) -> Result<String, InferenceError> {
        self.calls += 1;
        self.entered
            .send(samples.to_vec())
            .map_err(|_| InferenceError::Decode)?;
        if self.calls == 1 {
            self.release
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| InferenceError::Decode)?;
        }
        Ok(if self.calls == 1 {
            "provisional"
        } else {
            "revised final"
        }
        .into())
    }
}
fn feed(port: &mut crate::audio::CapturePort, samples: &[f32]) {
    for chunk in samples.chunks(320) {
        port.publish(chunk);
        thread::sleep(Duration::from_millis(1));
    }
}
fn guarded(
    runtime: MeasuredRuntime,
    release: Sender<()>,
    check: impl FnOnce(&MeasuredRuntime) -> TestResult,
) -> TestResult {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&runtime)));
    let _released = release.try_send(());
    let shutdown = runtime.shutdown();
    let joined = runtime.join();
    shutdown?;
    joined?;
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
#[test]
fn early_prefix_is_revised_by_an_always_whole_final_job() -> TestResult {
    let (entered, entries) = bounded(3);
    let (release, gate) = bounded(1);
    let runtime = MeasuredRuntime::spawn_full_prefix_revision(
        InferenceService::with_decoder(Held {
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
        let sid = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut whole = vec![0.1; 16000];
        whole.resize(24000, 0.0);
        feed(&mut input.port, &whole);
        let prefix = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(prefix, whole[..19200]);
        assert_eq!(runtime.snapshot()?.session.phase, Phase::Recording);
        assert_eq!(runtime.snapshot()?.session.text, None);
        assert_eq!(runtime.timing()?.t_text, None);
        let tail = vec![0.3125; 8137];
        feed(&mut input.port, &tail);
        whole.extend(tail);
        runtime.stop(sid)?;
        wait_state(runtime, |s| s.session.phase == Phase::Finalizing)?;
        assert!(entries.is_empty());
        assert_eq!(runtime.timing()?.t_text, None);
        release.send(())?;
        let final_pcm = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(
            final_pcm.len(),
            whole.len(),
            "final must include the prefix again"
        );
        assert_eq!(final_pcm, whole);
        let final_state = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(final_state.session.text.as_deref(), Some("revised final"));
        assert_eq!(final_state.session.outcome, Some(Outcome::Local));
        assert!(runtime.timing()?.t_text.is_some());
        let trace = runtime.job_trace()?;
        assert_eq!(trace.jobs.len(), 2);
        assert_eq!(trace.error, None);
        assert_eq!(
            (trace.jobs[1].timeline_start, trace.jobs[1].timeline_end),
            (0, whole.len())
        );
        assert!(
            trace
                .jobs
                .iter()
                .all(|job| job.adoption == Some("accepted"))
        );
        assert!(entries.is_empty());
        Ok(())
    })
}

#[test]
fn cancelled_old_snapshot_blocks_new_snapshots_until_actual_worker_release() -> TestResult {
    let (entered, entries) = bounded(4);
    let (release, gate) = bounded(1);
    let runtime = MeasuredRuntime::spawn_full_prefix_revision(
        InferenceService::with_decoder(Held {
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
    let trace_reader = runtime.job_trace_handle();
    guarded(runtime, release.clone(), |runtime| {
        wait_state(runtime, |s| s.model_ready)?;
        let old = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut voice = vec![0.1; 16000];
        voice.resize(24000, 0.0);
        feed(&mut input.port, &voice);
        assert_eq!(
            entries.recv_timeout(Duration::from_secs(2))?,
            voice[..19200]
        );
        runtime.cancel(old)?;
        wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        // Actual ordinary Client allows a new recording while the one revoked
        // native call finishes; measured starts intentionally require quiescence.
        await_value(
            runtime
                .runtime
                .client
                .start(settings(), None, false, false)?,
        )?
        .map_err(std::io::Error::other)?;
        let next = wait_state(runtime, |s| s.session.phase == Phase::Recording)?
            .session
            .session
            .ok_or("missing new SID")?;
        assert!(next > old);
        let mut input = runtime.input.recv_timeout(Duration::from_secs(2))?;
        feed(&mut input.port, &voice);
        feed(&mut input.port, &[0.0; 16000]);
        assert!(entries.recv_timeout(Duration::from_millis(60)).is_err());
        let current = runtime.job_trace()?;
        assert!(
            current.jobs.is_empty(),
            "old frozen storage + current journal exhaust the two-storage budget"
        );
        assert_eq!(current.retired_job.as_ref().map(|r| r.session), Some(old));
        assert_eq!(runtime.snapshot()?.session.phase, Phase::Recording);
        assert_eq!(runtime.snapshot()?.error, None);
        release.send(())?;
        assert_eq!(
            entries.recv_timeout(Duration::from_secs(2))?,
            voice[..19200]
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while runtime
            .job_trace()?
            .jobs
            .first()
            .is_none_or(|r| r.result_adopted_offset_nanos.is_none())
        {
            if Instant::now() >= deadline {
                return Err("new revision not adopted".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        feed(&mut input.port, &[0.0; 16000]);
        assert!(
            entries.recv_timeout(Duration::from_millis(60)).is_err(),
            "same safe cut must not be snapshotted again after result adoption"
        );
        assert_eq!(runtime.snapshot()?.session.text, None);
        runtime.runtime.client.stop(next)?;
        let final_pcm = entries.recv_timeout(Duration::from_secs(2))?;
        let mut whole = voice;
        whole.resize(56000, 0.0);
        assert_eq!(final_pcm, whole);
        let end = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(end.session.text.as_deref(), Some("revised final"));
        assert_eq!(end.session.outcome, Some(Outcome::Local));
        let trace = runtime.job_trace()?;
        assert_eq!(trace.jobs.len(), 2);
        assert!(trace.jobs.iter().all(|r| r.session == next));
        assert_eq!(
            trace.retired_job.as_ref().and_then(|r| r.worker_outcome),
            Some("cancelled-after-decode")
        );
        Ok(())
    })?;
    let after = trace_reader.snapshot()?;
    assert_eq!(after.error, None);
    assert!(
        after
            .retired_job
            .and_then(|r| r.worker_settled_offset_nanos)
            .is_some()
    );
    Ok(())
}

#[test]
fn cumulative_prefix_and_final_flush_match_the_continuous_44100_processor() -> TestResult {
    let (entered, entries) = bounded(3);
    let (release, gate) = bounded(1);
    let format = AudioFormat {
        sample_rate: 44100,
        channels: 1,
    };
    let runtime = MeasuredRuntime::spawn_full_prefix_revision(
        InferenceService::with_decoder(Held {
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
        let prefix = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(runtime.snapshot()?.session.phase, Phase::Recording);
        let end = vec![0.3125; 10003];
        feed(&mut input.port, &end);
        source.extend(end);
        runtime.stop(sid)?;
        wait_state(runtime, |s| s.session.phase == Phase::Finalizing)?;
        release.send(())?;
        let final_pcm = entries.recv_timeout(Duration::from_secs(2))?;
        let mut reference = dictation_core::audio::AudioProcessor::new(format)?;
        reference.push_interleaved(&source)?;
        let reference = reference.finish()?;
        assert_eq!(
            final_pcm, reference,
            "pause never resets or flushes the resampler"
        );
        assert_eq!(
            prefix,
            reference[..prefix.len()],
            "snapshot contains only irreversible stable frames"
        );
        assert_eq!(
            reference.len(),
            27629,
            "one final rounding/flush includes the partial end syllable"
        );
        let final_state = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(final_state.session.text.as_deref(), Some("revised final"));
        let trace = runtime.job_trace()?;
        assert_eq!(trace.jobs.len(), 2);
        assert!(
            trace
                .jobs
                .iter()
                .all(|r| r.timeline_start == 0 && r.storage_start == 0)
        );
        assert_eq!(trace.jobs[1].timeline_end, reference.len());
        Ok(())
    })
}

#[test]
fn backlog_coalesces_to_one_later_cumulative_prefix_without_repeating_silence() -> TestResult {
    let (entered, entries) = bounded(4);
    let (release, gate) = bounded(1);
    let runtime = MeasuredRuntime::spawn_full_prefix_revision(
        InferenceService::with_decoder(Held {
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
        let sid = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
        let mut input = runtime.take_input(Duration::from_secs(2))?;
        let mut whole = vec![0.1; 16000];
        whole.resize(24000, 0.0);
        feed(&mut input.port, &whole);
        assert_eq!(
            entries.recv_timeout(Duration::from_secs(2))?,
            whole[..19200]
        );
        let mut backlog = vec![0.2; 8000];
        backlog.resize(16000, 0.0);
        backlog.extend_from_slice(&[0.3; 8000]);
        backlog.resize(48000, 0.0);
        feed(&mut input.port, &backlog);
        whole.extend(backlog);
        assert!(entries.recv_timeout(Duration::from_millis(60)).is_err());
        assert_eq!(
            runtime.job_trace()?.jobs.len(),
            1,
            "backpressure retains only the latest safe-cut descriptor"
        );
        release.send(())?;
        let second = entries.recv_timeout(Duration::from_secs(2))?;
        assert_eq!(second.len(), 51200);
        assert_eq!(
            second,
            whole[..51200],
            "later previews revise all prior audio, not a disjoint suffix"
        );
        assert!(entries.recv_timeout(Duration::from_millis(60)).is_err());
        assert_eq!(runtime.timing()?.t_text, None);
        feed(&mut input.port, &[0.3125; 137]);
        whole.extend_from_slice(&[0.3125; 137]);
        runtime.stop(sid)?;
        assert_eq!(entries.recv_timeout(Duration::from_secs(2))?, whole);
        let end = wait_state(runtime, |s| s.session.phase == Phase::Idle)?;
        assert_eq!(end.session.text.as_deref(), Some("revised final"));
        let trace = runtime.job_trace()?;
        assert_eq!(trace.jobs.len(), 3);
        assert_eq!(trace.error, None);
        assert_eq!(
            trace
                .jobs
                .iter()
                .map(|r| (r.timeline_start, r.timeline_end))
                .collect::<Vec<_>>(),
            [(0, 19200), (0, 51200), (0, whole.len())]
        );
        assert_eq!(
            runtime
                .timing()?
                .decode_timing
                .ok_or("missing actual accumulated work")?
                .input_frames,
            19200 + 51200 + whole.len(),
            "cumulative decoder work is not unique recording coverage"
        );
        Ok(())
    })
}
