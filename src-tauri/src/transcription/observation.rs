//! Developer-only accepted native segments; missing/dropped encoder windows remain unknown.
//! Native times under the unchanged no-timestamps configuration can be negative
//! or reversed and are not usable as alignment. Preserve them verbatim; this is
//! observation metadata, not a production transcription failure.
//! At most 4096 records and 256 KiB of raw segment text are retained, in addition
//! to the normal bounded joined transcript. No audio samples are retained here.
use super::InferenceError;

pub const MAX_SEGMENTS: usize = 4096;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;

#[derive(Debug, PartialEq)]
pub struct Segment {
    pub index: i32,
    /// Signed native timestamp in centiseconds. With no_timestamps=true this
    /// may be unusable; it is neither repaired nor guaranteed relative alignment.
    pub start_centiseconds: i64,
    pub end_centiseconds: i64,
    /// Only start >= 0 && end >= start; true does not establish audio alignment.
    pub nonnegative_ordered_timestamps: bool,
    pub no_speech_probability: f32,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Observation {
    pub input_frames: usize,
    /// End-exclusive range in the original 16-kHz PCM, before any zero padding.
    pub selected_range: Option<std::ops::Range<usize>>,
    pub selected_frames: usize,
    /// Actual ASR input including any short-input zero padding; zero if gated out.
    pub asr_input_frames: usize,
    /// Number of appended zero frames, not total ASR input length.
    pub padded_frames: usize,
    pub segments: Vec<Segment>,
    text_bytes: usize,
}

impl Observation {
    pub(super) fn record(
        &mut self,
        index: i32,
        start: i64,
        end: i64,
        probability: f32,
        text: &str,
    ) -> Result<(), InferenceError> {
        let bytes = self
            .text_bytes
            .checked_add(text.len())
            .ok_or(InferenceError::Observation)?;
        if self.segments.len() >= MAX_SEGMENTS || bytes > MAX_TEXT_BYTES {
            return Err(InferenceError::Observation);
        }
        if index < 0
            || index as usize != self.segments.len()
            || !probability.is_finite()
            || !(0.0..=1.0).contains(&probability)
        {
            return Err(InferenceError::ObservationMetadata {
                index,
                start,
                end,
                probability,
            });
        }
        self.segments
            .try_reserve(1)
            .map_err(|_| InferenceError::Memory)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(text.len())
            .map_err(|_| InferenceError::Memory)?;
        owned.push_str(text);
        self.segments.push(Segment {
            index,
            start_centiseconds: start,
            end_centiseconds: end,
            nonnegative_ordered_timestamps: start >= 0 && end >= start,
            no_speech_probability: probability,
            text: owned,
        });
        self.text_bytes = bytes;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ObservedTranscription {
    pub text: String,
    pub observation: Observation,
}

impl super::ObservationSink for Observation {
    fn prepared(
        &mut self,
        frames: usize,
        range: Option<std::ops::Range<usize>>,
        asr_frames: usize,
    ) {
        self.input_frames = frames;
        self.selected_frames = range.as_ref().map_or(0, |r| r.len());
        self.selected_range = range;
        self.asr_input_frames = asr_frames;
        self.padded_frames = asr_frames.saturating_sub(self.selected_frames);
    }

    fn segment(
        &mut self,
        segment: &whisper_rs::WhisperSegment<'_>,
        text: &str,
    ) -> Result<(), InferenceError> {
        self.record(
            segment.segment_index(),
            segment.start_timestamp(),
            segment.end_timestamp(),
            segment.no_speech_probability(),
            text,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_native_segments_preserve_raw_text_times_and_probability()
    -> Result<(), InferenceError> {
        let mut observation = Observation::default();
        observation.record(0, 0, 2346, 0.125, " First. First. ")?;
        assert_eq!(observation.segments.len(), 1);
        let segment = &observation.segments[0];
        assert_eq!(segment.text, " First. First. ");
        assert_eq!(
            (
                segment.index,
                segment.start_centiseconds,
                segment.end_centiseconds
            ),
            (0, 0, 2346)
        );
        assert_eq!(segment.no_speech_probability, 0.125);
        Ok(())
    }
    #[test]
    fn invalid_native_metadata_is_an_error_not_an_empty_observation() -> Result<(), InferenceError>
    {
        for (index, start, end, probability) in [
            (-1, 0, 1, 0.0),
            (1, 0, 1, 0.0),
            (0, 0, 1, f32::NAN),
            (0, 0, 1, f32::INFINITY),
            (0, 0, 1, -0.01),
            (0, 0, 1, 1.01),
        ] {
            let mut observation = Observation::default();
            assert!(matches!(
                observation.record(index, start, end, probability, "bad"),
                Err(InferenceError::ObservationMetadata { .. })
            ));
            assert!(observation.segments.is_empty());
            observation.record(0, 0, 1, 0.5, "valid after rejected record")?;
            assert_eq!(observation.segments.len(), 1);
        }
        Ok(())
    }

    #[test]
    fn native_segment_and_utf8_byte_budgets_are_inclusive_and_fail_explicitly()
    -> Result<(), InferenceError> {
        let mut count = Observation::default();
        for index in 0..4096 {
            count.record(index, 0, 1, 0.0, "")?;
        }
        assert!(matches!(
            count.record(4096, 0, 1, 0.0, ""),
            Err(InferenceError::Observation)
        ));
        assert_eq!(count.segments.len(), 4096);
        let mut bytes = Observation::default();
        // UTF-8 bytes, not characters: 131072 two-byte characters exhaust 256 KiB.
        let full = "ä".repeat(131072);
        bytes.record(0, 0, 1, 1.0, &full)?;
        assert!(matches!(
            bytes.record(1, 1, 2, 1.0, "x"),
            Err(InferenceError::Observation)
        ));
        assert_eq!(bytes.segments.len(), 1);
        assert_eq!(bytes.segments[0].text, full);
        Ok(())
    }
    #[test]
    fn invalid_native_metadata_retains_actual_numbers_without_segment_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut observation = Observation::default();
        let result = observation.record(7, -17, 2346, 0.125, "private text must not escape");
        let display = result.as_ref().err().ok_or("expected failure")?.to_string();
        assert!(display.contains("index=7, start=-17, end=2346, probability=0.125"));
        assert!(!display.contains("private text"));
        match result {
            Err(InferenceError::ObservationMetadata {
                index,
                start,
                end,
                probability,
            }) => {
                assert_eq!(index, 7);
                assert_eq!(start, -17);
                assert_eq!(end, 2346);
                assert_eq!(probability, 0.125);
            }
            other => {
                return Err(format!("expected typed native metadata error, got {other:?}").into());
            }
        }
        let result = observation.record(0, 0, 1, f32::NAN, "private text");
        assert!(
            matches!(result, Err(InferenceError::ObservationMetadata { index: 0, start: 0, end: 1, probability }) if probability.is_nan())
        );
        Ok(())
    }
    #[test]
    fn no_timestamps_native_negative_start_is_preserved_not_repaired() -> Result<(), InferenceError>
    {
        let mut observation = Observation::default();
        observation.record(0, -100730, 3000, 7e-12, " exact raw text ")?;
        let segment = &observation.segments[0];
        assert_eq!(segment.start_centiseconds, -100730);
        assert_eq!(segment.end_centiseconds, 3000);
        assert_eq!(segment.text, " exact raw text ");
        assert!(!segment.nonnegative_ordered_timestamps);
        Ok(())
    }

    #[test]
    fn timestamp_flag_is_only_arithmetic_and_keeps_reversed_raw_times() -> Result<(), InferenceError>
    {
        let mut observation = Observation::default();
        for (index, (start, end, expected)) in [
            (0, 0, true),
            (10, 20, true),
            (20, 10, false),
            (-2, -1, false),
            (i64::MIN, i64::MAX, false),
        ]
        .into_iter()
        .enumerate()
        {
            observation.record(index as i32, start, end, 0.0, "")?;
            let segment = &observation.segments[index];
            assert_eq!(
                (segment.start_centiseconds, segment.end_centiseconds),
                (start, end)
            );
            assert_eq!(segment.nonnegative_ordered_timestamps, expected);
        }
        Ok(())
    }
}
