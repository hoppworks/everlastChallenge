use dictation_core::audio::{AudioFormat, AudioProcessor};

fn tone(rate: u32, frequency: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| (i as f32 * std::f32::consts::TAU * frequency / rate as f32).sin() * 0.2)
        .collect()
}

fn amplitude(signal: &[f32], frequency: f64) -> f64 {
    let (sin, cos) = signal
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(sin, cos), (i, sample)| {
            let phase = i as f64 * std::f64::consts::TAU * frequency / 16_000.0;
            (
                sin + f64::from(*sample) * phase.sin(),
                cos + f64::from(*sample) * phase.cos(),
            )
        });
    2.0 * sin.hypot(cos) / signal.len() as f64
}

#[test]
fn supported_rates_preserve_duration_passband_and_chunk_continuity()
-> Result<(), Box<dyn std::error::Error>> {
    for rate in [8_000, 16_000, 24_000, 32_000, 44_100, 48_000] {
        let input = tone(rate, 1_000.0, rate as usize + 7);
        let mut processor = AudioProcessor::new(AudioFormat {
            sample_rate: rate,
            channels: 1,
        })?;
        for chunk in input.chunks(317) {
            processor.push_interleaved(chunk)?;
        }
        let output = processor.finish()?;
        assert_eq!(
            output.len(),
            (input.len() * 16_000 + rate as usize / 2) / rate as usize
        );
        let gain = 20.0 * (amplitude(&output[1_024..9_024], 1_000.0) / 0.2).log10();
        assert!(gain.abs() <= 1.0, "passband at {rate}: {gain} dB");
    }
    Ok(())
}

#[test]
fn filtering_rejects_downsample_aliases_and_upsample_images()
-> Result<(), Box<dyn std::error::Error>> {
    for (rate, input_frequency, unwanted_frequency) in [
        (44_100, 12_000.0, 4_000.0),
        (48_000, 12_000.0, 4_000.0),
        (8_000, 1_000.0, 7_000.0),
    ] {
        let input = tone(rate, input_frequency, rate as usize);
        let mut processor = AudioProcessor::new(AudioFormat {
            sample_rate: rate,
            channels: 1,
        })?;
        processor.push_interleaved(&input)?;
        let output = processor.finish()?;
        let suppression =
            20.0 * (amplitude(&output[1_024..9_024], unwanted_frequency) / 0.2).log10();
        assert!(
            suppression <= -40.0,
            "unwanted component at {rate}: {suppression} dB"
        );
    }
    Ok(())
}

#[test]
fn invalid_formats_and_samples_fail_without_manufactured_audio()
-> Result<(), Box<dyn std::error::Error>> {
    use dictation_core::audio::AudioError;
    for format in [
        AudioFormat {
            sample_rate: 0,
            channels: 1,
        },
        AudioFormat {
            sample_rate: 96_000,
            channels: 1,
        },
        AudioFormat {
            sample_rate: 16_000,
            channels: 0,
        },
        AudioFormat {
            sample_rate: 16_000,
            channels: 9,
        },
    ] {
        assert!(matches!(
            AudioProcessor::new(format),
            Err(AudioError::UnsupportedFormat)
        ));
    }
    let mut processor = AudioProcessor::new(AudioFormat {
        sample_rate: 16_000,
        channels: 2,
    })?;
    assert!(matches!(
        processor.push_interleaved(&[0.2]),
        Err(AudioError::IncompleteFrame)
    ));
    assert!(matches!(
        processor.push_interleaved(&[f32::NAN, 0.0]),
        Err(AudioError::NonFinite)
    ));
    assert!(processor.finish()?.is_empty());
    let processor = AudioProcessor::new(AudioFormat {
        sample_rate: 48_000,
        channels: 1,
    })?;
    assert!(processor.finish()?.is_empty());
    Ok(())
}

#[test]
fn journal_has_a_hard_five_minute_boundary() -> Result<(), Box<dyn std::error::Error>> {
    use dictation_core::audio::{AudioError, MAX_SAMPLES};
    let mut processor = AudioProcessor::new(AudioFormat {
        sample_rate: 16_000,
        channels: 1,
    })?;
    let second = vec![0.1; 16_000];
    for _ in 0..300 {
        processor.push_interleaved(&second)?;
    }
    assert!(matches!(
        processor.push_interleaved(&[0.1]),
        Err(AudioError::DurationLimit)
    ));
    assert_eq!(processor.finish()?.len(), MAX_SAMPLES);
    Ok(())
}
