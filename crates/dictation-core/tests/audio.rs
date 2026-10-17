use dictation_core::audio::{AudioFormat, AudioProcessor};

#[test]
fn silence_is_empty_but_a_short_quiet_utterance_retains_its_edges() {
    use dictation_core::audio::{EnergyProfile, speech_range};
    let profile = EnergyProfile::default();
    assert_eq!(speech_range(&vec![0.0; 80_000], profile), None);
    let quantization_floor: Vec<f32> = (0..80_000)
        .map(|index| if index % 2 == 0 { 1.0 } else { -1.0 } / 32_768.0)
        .collect();
    assert_eq!(speech_range(&quantization_floor, profile), None);
    let mut signal = vec![0.0; 48_000];
    signal[16_000..19_200].fill(0.02);
    assert_eq!(speech_range(&signal, profile), Some(13_600..22_400));
}

#[test]
fn quiet_pcm16_signal_is_not_discarded_before_inference() {
    use dictation_core::audio::{EnergyProfile, speech_range};
    let mut signal = vec![0.0; 48_000];
    // Approximately the weakest measured calibration clip, quantized like the
    // production fixtures. This checks energy eligibility, not semantic speech.
    for (index, sample) in signal[16_000..19_200].iter_mut().enumerate() {
        let wave = (index as f32 * std::f32::consts::TAU * 1_000.0 / 16_000.0).sin();
        *sample = (wave * 5.0).round() / 32_768.0;
    }
    // Even softer onset and a 300-ms signal tail sit below the start
    // threshold. Preroll and the lower end threshold must retain both.
    for range in [14_400..16_000, 19_200..24_000] {
        for (index, sample) in signal[range].iter_mut().enumerate() {
            let wave = (index as f32 * std::f32::consts::TAU * 1_000.0 / 16_000.0).sin();
            *sample = (wave * 2.0).round() / 32_768.0;
        }
    }
    assert_eq!(
        speech_range(&signal, EnergyProfile::default()),
        Some(13_600..27_200)
    );
}

#[test]
fn flush_preserves_a_short_stereo_utterance_at_device_rate()
-> Result<(), Box<dyn std::error::Error>> {
    let mut processor = AudioProcessor::new(AudioFormat {
        sample_rate: 48_000,
        channels: 2,
    })?;
    let signal: Vec<f32> = (0..4_800)
        .flat_map(|i| {
            let sample = (i as f32 * std::f32::consts::TAU * 1_000.0 / 48_000.0).sin() * 0.2;
            [sample, sample]
        })
        .collect();
    for block in signal.chunks(514) {
        processor.push_interleaved(block)?;
    }
    let result = processor.finish()?;
    assert_eq!(result.len(), 1_600);
    let rms = (result[160..1_440].iter().map(|x| x * x).sum::<f32>() / 1_280.0).sqrt();
    assert!(
        (rms - 0.2 / std::f32::consts::SQRT_2).abs() < 0.01,
        "unexpected passband amplitude {rms}"
    );
    Ok(())
}
