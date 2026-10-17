//! Developer-only accepted native segments; missing/dropped encoder windows remain unknown.
//! Native times under the unchanged no-timestamps configuration are not word alignment.
//! At most 4096 records and 256 KiB of raw segment text are retained, in addition
//! to the normal bounded joined transcript. No audio samples are retained here.
use super::InferenceError;

pub const MAX_SEGMENTS: usize = 4096;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;

#[derive(Debug, PartialEq)]
pub struct Segment {
    pub index: i32,
    /// Native timestamps in centiseconds, relative to the selected ASR input.
    pub start_centiseconds: i64,
    pub end_centiseconds: i64,
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
        if self.segments.len() >= MAX_SEGMENTS
            || bytes > MAX_TEXT_BYTES
            || index < 0
            || index as usize != self.segments.len()
            || start < 0
            || end < start
            || !probability.is_finite()
            || !(0.0..=1.0).contains(&probability)
        {
            return Err(InferenceError::Observation);
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
            (0, -1, 1, 0.0),
            (0, 2, 1, 0.0),
            (0, 0, 1, f32::NAN),
            (0, 0, 1, f32::INFINITY),
            (0, 0, 1, -0.01),
            (0, 0, 1, 1.01),
        ] {
            let mut observation = Observation::default();
            assert!(matches!(
                observation.record(index, start, end, probability, "bad"),
                Err(InferenceError::Observation)
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
}
