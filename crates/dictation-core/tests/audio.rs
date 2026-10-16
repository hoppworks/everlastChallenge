use dictation_core::audio::{AudioFormat, AudioProcessor};

#[test]
fn silence_is_empty_but_a_short_quiet_utterance_retains_its_edges() {
    use dictation_core::audio::{EnergyProfile, speech_range};
    let profile = EnergyProfile::default();
    assert_eq!(speech_range(&vec![0.0; 80_000], profile), None);
    let mut signal = vec![0.0; 48_000];
    signal[16_000..19_200].fill(0.02);
    assert_eq!(speech_range(&signal, profile), Some(13_600..22_400));
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
