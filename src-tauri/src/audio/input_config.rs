//! Deterministic device negotiation. Runs on the audio owner before allocation/callbacks.
use super::{AudioFormat, CaptureError};
use cpal::{SupportedStreamConfig, SupportedStreamConfigRange};

const RATES: [u32; 6] = [48_000, 44_100, 32_000, 24_000, 16_000, 8_000];

fn sample_rank(format: cpal::SampleFormat) -> Option<usize> {
    use cpal::SampleFormat::*;
    [F32, I16, I32, F64, I8, I64, U8, U16, U32, U64]
        .iter()
        .position(|value| *value == format)
}
fn supported(config: &SupportedStreamConfig) -> bool {
    AudioFormat {
        sample_rate: config.sample_rate().0,
        channels: config.channels(),
    }
    .validate()
    .is_ok()
        && sample_rank(config.sample_format()).is_some()
}

pub(super) fn choose(
    default: Option<SupportedStreamConfig>,
    ranges: impl IntoIterator<Item = SupportedStreamConfigRange>,
) -> Result<SupportedStreamConfig, CaptureError> {
    if let Some(config) = default.filter(supported) {
        return Ok(config);
    }
    // Prefer fewer channels, then the highest already-qualified rate, then
    // native float/signed formats. No DSP capability is added by negotiation.
    ranges
        .into_iter()
        .flat_map(|range| {
            RATES
                .into_iter()
                .filter_map(move |rate| range.try_with_sample_rate(cpal::SampleRate(rate)))
        })
        .filter(supported)
        .min_by_key(|config| {
            let buffer = match config.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => (0, *min, *max),
                cpal::SupportedBufferSize::Unknown => (1, 0, 0),
            };
            (
                config.channels(),
                RATES
                    .iter()
                    .position(|rate| *rate == config.sample_rate().0),
                sample_rank(config.sample_format()),
                buffer,
            )
        })
        .ok_or(CaptureError::UnsupportedSampleFormat)
}

pub(super) fn negotiate<I: IntoIterator<Item = SupportedStreamConfigRange>>(
    default: Result<SupportedStreamConfig, String>,
    advertised: impl FnOnce() -> Result<I, String>,
) -> Result<SupportedStreamConfig, CaptureError> {
    if let Ok(config) = &default
        && supported(config)
    {
        return Ok(config.clone());
    }
    let ranges = advertised().map_err(|error| {
        CaptureError::Native(match &default {
            Err(default_error) => {
                format!("Standardformat: {default_error}; verfügbare Formate: {error}")
            }
            Ok(_) => format!("Verfügbare Mikrofonformate konnten nicht ermittelt werden: {error}"),
        })
    })?;
    choose(default.ok(), ranges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::{SampleFormat, SampleRate, SupportedBufferSize};
    fn range(channels: u16, low: u32, high: u32) -> SupportedStreamConfigRange {
        SupportedStreamConfigRange::new(
            channels,
            SampleRate(low),
            SampleRate(high),
            SupportedBufferSize::Unknown,
            SampleFormat::F32,
        )
    }
    #[test]
    fn incompatible_default_uses_an_advertised_supported_alternative() -> Result<(), CaptureError> {
        let default = range(2, 96_000, 96_000).with_max_sample_rate();
        let selected = choose(Some(default), [range(2, 44_100, 48_000)])?;
        assert_eq!(selected.sample_rate().0, 48_000);
        assert_eq!(selected.channels(), 2);
        Ok(())
    }
    #[test]
    fn compatible_default_does_not_need_a_successful_device_enumeration() -> Result<(), CaptureError>
    {
        let default = range(2, 44_100, 44_100).with_max_sample_rate();
        let chosen = negotiate(
            Ok(default.clone()),
            || -> Result<Vec<SupportedStreamConfigRange>, String> {
                Err("enumeration must not be necessary".into())
            },
        )?;
        assert_eq!(chosen.sample_rate(), default.sample_rate());
        assert_eq!(chosen.channels(), 2);
        Ok(())
    }
    #[test]
    fn order_independent_selection_prefers_fewer_channels_then_rate() -> Result<(), CaptureError> {
        let a = range(2, 48_000, 48_000);
        let b = range(1, 16_000, 48_000);
        for ranges in [[a, b], [b, a]] {
            let chosen = choose(None, ranges)?;
            assert_eq!(chosen.channels(), 1);
            assert_eq!(chosen.sample_rate().0, 48_000);
        }
        Ok(())
    }
    #[test]
    fn failed_default_can_use_advertised_format_but_query_errors_are_not_hidden()
    -> Result<(), CaptureError> {
        let selected = negotiate(Err("no default".into()), || Ok([range(1, 16_000, 16_000)]))?;
        assert_eq!(selected.sample_rate().0, 16_000);
        let result = negotiate(
            Err("no default".into()),
            || -> Result<Vec<SupportedStreamConfigRange>, String> { Err("device removed".into()) },
        );
        assert!(
            matches!(result, Err(CaptureError::Native(message)) if message.contains("no default") && message.contains("device removed"))
        );
        Ok(())
    }
    #[test]
    fn invalid_channels_and_rates_are_rejected() {
        for invalid in [
            range(0, 48_000, 48_000),
            range(9, 48_000, 48_000),
            range(1, 96_000, 96_000),
        ] {
            assert!(choose(None, [invalid]).is_err());
        }
        assert!(choose(None, []).is_err());
    }
}
