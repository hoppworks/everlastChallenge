use dictation_core::recording_limits::{LimitDecision, RecordingLimits};
use std::time::Duration;

#[test]
fn warning_occurs_once_at_exactly_270_seconds() {
    let mut limits = RecordingLimits::default();
    assert_eq!(
        limits.advance(Duration::ZERO, false),
        LimitDecision::Continue
    );
    assert_eq!(
        limits.advance(Duration::from_nanos(269_999_999_999), false),
        LimitDecision::Continue
    );
    assert_eq!(
        limits.advance(Duration::from_secs(270), false),
        LimitDecision::Warn
    );
    assert_eq!(
        limits.advance(Duration::from_secs(270), false),
        LimitDecision::Continue
    );
    assert_eq!(
        limits.advance(Duration::from_secs(299), false),
        LimitDecision::Continue
    );
}

#[test]
fn exact_deadline_or_delayed_first_poll_stops_permanently() {
    let mut limits = RecordingLimits::default();
    assert_eq!(
        limits.advance(Duration::from_secs(270), false),
        LimitDecision::Warn
    );
    assert_eq!(
        limits.advance(Duration::from_nanos(299_999_999_999), false),
        LimitDecision::Continue
    );
    assert_eq!(
        limits.advance(Duration::from_secs(300), false),
        LimitDecision::Stop
    );
    assert_eq!(limits.advance(Duration::ZERO, false), LimitDecision::Stop);
    for first_poll in [300, 301, 3_600] {
        let mut delayed = RecordingLimits::default();
        assert_eq!(
            delayed.advance(Duration::from_secs(first_poll), false),
            LimitDecision::Stop
        );
        assert_eq!(
            delayed.advance(Duration::from_secs(270), false),
            LimitDecision::Stop
        );
    }
}

#[test]
fn frame_capacity_stops_early_and_a_fresh_session_resets_both_limits() {
    for early in [0, 1, 269, 270, 299] {
        let mut limits = RecordingLimits::default();
        assert_eq!(
            limits.advance(Duration::from_secs(early), true),
            LimitDecision::Stop
        );
        assert_eq!(limits.advance(Duration::ZERO, false), LimitDecision::Stop);
    }
    let mut previous = RecordingLimits::default();
    assert_eq!(
        previous.advance(Duration::from_secs(270), false),
        LimitDecision::Warn
    );
    assert_eq!(
        previous.advance(Duration::from_secs(300), false),
        LimitDecision::Stop
    );
    let mut next = RecordingLimits::default();
    assert_eq!(next.advance(Duration::ZERO, false), LimitDecision::Continue);
    assert_eq!(
        next.advance(Duration::from_secs(270), false),
        LimitDecision::Warn
    );
}
