//! Clocked input validation; every failure after initialization returns evidence.
use crossbeam_channel::bounded;
use dictation_core::audio::AudioFormat;
use everlast_dictation::audio::{
    CaptureEvent, CaptureHandle,
    replay::schedule::{JITTER_TOLERANCE, PublicationTiming, ReplaySchedule, observe},
};
use serde::Serialize;
use std::{
    thread,
    time::{Duration, Instant},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    pub start_frame: usize,
    pub end_frame: usize,
    pub planned_nanos: u128,
    pub before_publish_nanos: u128,
    pub after_publish_nanos: u128,
    pub deviation_nanos: u128,
    pub timing: &'static str,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub passed: bool,
    pub timing_valid: bool,
    pub started_events: usize,
    pub finished_events: usize,
    pub started_clock_verified: bool,
    pub input_output_samples_equal: bool,
    pub planned_stop_offset_nanos: Option<u128>,
    pub audio_owner_stop_request_offset_nanos: Option<u128>,
    pub audio_finished_observed_offset_nanos: Option<u128>,
    pub stop_requests: usize,
    pub observation_failure: Option<String>,
    pub audio_failure: Option<String>,
    pub joined: bool,
    pub join_unverified: bool,
    pub join_failure: Option<String>,
    pub publications: Vec<Publication>,
}

/// Short sleeps avoid relying on one coalescible 20-ms OS timer. The final
/// spin is bounded by 250 us per chunk; it does not wait for a consumer.
fn wait_until(target: Instant) {
    loop {
        let remaining = target.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        if remaining <= Duration::from_micros(250) {
            std::hint::spin_loop();
        } else {
            thread::sleep((remaining - Duration::from_micros(250)).min(Duration::from_millis(1)));
        }
    }
}

fn drain_events(
    handle: &CaptureHandle,
    run: &mut Run,
    t0: Instant,
    samples: &[f32],
    format: AudioFormat,
) {
    while let Some(event) = handle.next_event() {
        match event {
            CaptureEvent::Started {
                session,
                started,
                format: actual,
            } => {
                run.started_events += 1;
                run.started_clock_verified = session == 1 && started == t0 && actual == format;
            }
            CaptureEvent::Finished {
                session, result, ..
            } => {
                run.finished_events += 1;
                run.audio_finished_observed_offset_nanos = Some(t0.elapsed().as_nanos());
                match result {
                    Ok(audio) => run.input_output_samples_equal = session == 1 && audio == samples,
                    Err(error) => run.audio_failure = Some(error.to_string()),
                }
            }
            CaptureEvent::MaximumWarning { .. } => {}
        }
    }
}

fn join_if_finished(handle: CaptureHandle, run: &mut Run) {
    // Never block on an owner that this observer has not seen finish. Drop
    // still requests Stop; an unobserved join is explicitly a failed run.
    if !handle.is_finished() {
        run.join_unverified = true;
        return;
    }
    match handle.join() {
        Ok(()) => run.joined = true,
        Err(error) => run.join_failure = Some(error.to_string()),
    }
}

pub fn run(samples: &[f32], finish_budget: Duration) -> Run {
    let format = AudioFormat {
        sample_rate: 16_000,
        channels: 1,
    };
    let mut run = Run::default();
    let schedule = match ReplaySchedule::new(format, samples.len()) {
        Ok(schedule) => schedule,
        Err(error) => {
            run.observation_failure = Some(error.to_string());
            return run;
        }
    };
    run.publications.reserve_exact(schedule.chunks().len());
    run.planned_stop_offset_nanos = schedule
        .chunks()
        .last()
        .map(|plan| plan.planned_offset.as_nanos());
    let (ready, input_ready) = bounded(1);
    let handle = match CaptureHandle::start_replay(1, format, ready) {
        Ok(handle) => handle,
        Err(error) => {
            run.observation_failure = Some(error.to_string());
            return run;
        }
    };
    let mut source = match input_ready.recv_timeout(Duration::from_secs(10)) {
        Ok(source) => source,
        Err(error) => {
            run.observation_failure = Some(format!("replay readiness observation failed: {error}"));
            handle.request_stop();
            run.stop_requests += 1;
            join_if_finished(handle, &mut run);
            return run;
        }
    };
    run.timing_valid = source.format == format;
    if source.format != format {
        run.observation_failure = Some("replay source format changed".into());
    } else {
        for plan in schedule.chunks() {
            wait_until(source.started + plan.planned_offset);
            let before = source.started.elapsed();
            source
                .port
                .publish(&samples[plan.start_frame..plan.end_frame]);
            let after = source.started.elapsed();
            let observation = observe(plan, after);
            let timing = match observation.classification {
                PublicationTiming::Early => "early",
                PublicationTiming::OnBudget => "onBudget",
                PublicationTiming::Late => "late",
            };
            run.timing_valid &= before >= plan.planned_offset
                && observation.classification == PublicationTiming::OnBudget;
            run.publications.push(Publication {
                start_frame: plan.start_frame,
                end_frame: plan.end_frame,
                planned_nanos: plan.planned_offset.as_nanos(),
                before_publish_nanos: before.as_nanos(),
                after_publish_nanos: after.as_nanos(),
                deviation_nanos: observation.deviation.as_nanos(),
                timing,
            });
        }
    }
    let stop = source.started.elapsed();
    run.audio_owner_stop_request_offset_nanos = Some(stop.as_nanos());
    handle.request_stop();
    run.stop_requests += 1;
    drop(source.port);
    let planned_stop = schedule
        .chunks()
        .last()
        .map_or(Duration::ZERO, |plan| plan.planned_offset);
    run.timing_valid &= stop >= planned_stop && stop - planned_stop <= JITTER_TOLERANCE;
    let deadline = Instant::now() + finish_budget;
    loop {
        if Instant::now() >= deadline {
            run.observation_failure
                .get_or_insert_with(|| "audio-owner finish observation timed out".into());
            break;
        }
        drain_events(&handle, &mut run, source.started, samples, format);
        if handle.is_finished() {
            // Close the last-event/is_finished race before joining.
            drain_events(&handle, &mut run, source.started, samples, format);
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    join_if_finished(handle, &mut run);
    run.passed = run.timing_valid
        && run.started_events == 1
        && run.finished_events == 1
        && run.started_clock_verified
        && run.input_output_samples_equal
        && run.joined
        && run.audio_failure.is_none()
        && run.join_failure.is_none()
        && run.observation_failure.is_none();
    run
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_timeout_keeps_every_already_published_chunk_and_serializes_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        // Zero observation budget deterministically expires before draining,
        // even if the real audio owner happens to finish first.
        let result = run(&[0.125; 320], Duration::ZERO);
        assert!(!result.passed);
        assert!(
            result
                .observation_failure
                .as_deref()
                .is_some_and(|error| error.contains("timed out"))
        );
        assert_eq!(result.publications.len(), 1);
        assert_eq!(result.publications[0].end_frame, 320);
        assert_eq!(result.publications[0].planned_nanos, 20_000_000);
        assert_eq!(result.stop_requests, 1);
        let json = serde_json::to_value(&result)?;
        assert_eq!(json["publications"].as_array().map(Vec::len), Some(1));
        assert!(json["observationFailure"].is_string());
        assert_eq!(json["passed"], false);
        Ok(())
    }

    #[test]
    fn real_capture_pipeline_reports_invalid_samples_without_losing_timing_evidence() {
        let result = run(&[f32::NAN; 320], Duration::from_secs(2));
        assert!(!result.passed);
        assert_eq!(result.publications.len(), 1);
        assert!(
            result
                .audio_failure
                .as_deref()
                .is_some_and(|error| error.contains("invalid samples"))
        );
        assert!(result.joined);
    }

    #[test]
    fn replay_uses_exact_frames_and_one_stop_without_early_publication() {
        let result = run(&[0.125; 321], Duration::from_secs(2));
        // OS jitter may invalidate a setup; never weaken the time budget to
        // make this test green. Data integrity and no-early input are invariant.
        assert_eq!(result.publications.len(), 2);
        assert_eq!(result.publications[0].end_frame, 320);
        assert_eq!(result.publications[1].end_frame, 321);
        assert_eq!(result.publications[1].planned_nanos, 20_062_500);
        assert!(
            result
                .publications
                .iter()
                .all(|row| row.before_publish_nanos >= row.planned_nanos)
        );
        assert_eq!(result.stop_requests, 1);
        assert!(result.input_output_samples_equal && result.joined);
        assert!(result.observation_failure.is_none());
    }
}
