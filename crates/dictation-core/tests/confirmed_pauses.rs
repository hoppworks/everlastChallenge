use dictation_core::audio::{AudioFormat, AudioProcessor, MAX_SAMPLES, pauses::ConfirmedPauses};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn confirmation_is_500ms_with_tail_and_no_repeated_silence_jobs() {
    let mut rule = ConfirmedPauses::default();
    let mut audio = vec![0.1; 16000];
    audio.resize(23984, 0.0); // 499 ms after the voice window ended.
    assert_eq!(rule.observe(0, &audio), None);
    audio.resize(24000, 0.0);
    assert_eq!(rule.observe(0, &audio), Some(19200));
    audio.resize(100000, 0.0);
    assert_eq!(rule.observe(0, &audio), Some(19200));
    // The descriptor is stable, not a growing list of empty jobs.
    assert_eq!(rule.observe(0, &audio), Some(19200));
}

#[test]
fn voice_hysteresis_and_partial_window_survive_callback_and_seal_boundaries() -> TestResult {
    let mut dsp = AudioProcessor::new(AudioFormat {
        sample_rate: 16000,
        channels: 1,
    })?;
    let mut rule = ConfirmedPauses::default();
    dsp.push_interleaved(&[0.00006; 320])?; // crosses start threshold
    let (base, samples) = dsp.stable_samples();
    assert_eq!(rule.observe(base, samples), None);
    dsp.push_interleaved(&[0.00003; 320])?; // below start, still above end
    dsp.push_interleaved(&[0.0; 7999])?;
    let (base, samples) = dsp.stable_samples();
    assert_eq!(rule.observe(base, samples), None);
    dsp.push_interleaved(&[0.0])?;
    let (base, samples) = dsp.stable_samples();
    assert_eq!(rule.observe(base, samples), Some(3840));
    let frozen = dsp.seal_prefix(3840)?;
    assert_eq!(frozen.len(), 8640); // physical suffix retained, decoder uses prefix only.
    dsp.push_interleaved(&[0.2; 17])?; // final partial energy window remains PCM
    let (base, samples) = dsp.stable_samples();
    assert_eq!(rule.observe(base, samples), Some(3840));
    let rest = dsp.finish()?;
    let mut restored = frozen[..3840].to_vec();
    restored.extend(rest);
    assert_eq!(restored.len(), 8657);
    assert_eq!(&restored[8640..], &[0.2; 17]);
    Ok(())
}

#[test]
fn sealed_44100_output_and_final_flush_equal_the_unsegmented_dsp_exactly() -> TestResult {
    let format = AudioFormat {
        sample_rate: 44100,
        channels: 1,
    };
    let input: Vec<f32> = (0..100003)
        .map(|i| {
            if !(22050..66150).contains(&i) {
                ((i % 97) as f32 / 97.0 - 0.5) * 0.4
            } else {
                0.0
            }
        })
        .collect();
    let mut reference = AudioProcessor::new(format)?;
    let mut segmented = AudioProcessor::new(format)?;
    let mut rule = ConfirmedPauses::default();
    let mut prefixes = Vec::new();
    for chunk in input.chunks(397) {
        reference.push_interleaved(chunk)?;
        segmented.push_interleaved(chunk)?;
        let (base, stable) = segmented.stable_samples();
        if let Some(cut) = rule.observe(base, stable).filter(|cut| *cut > base) {
            let frozen = segmented.seal_prefix(cut)?;
            prefixes.extend_from_slice(&frozen[..cut - base]);
        }
    }
    assert!(
        !prefixes.is_empty(),
        "fixture must actually exercise a seal before final flush"
    );
    prefixes.extend(segmented.finish()?);
    assert_eq!(prefixes, reference.finish()?);
    Ok(())
}

#[test]
fn uninterrupted_300_seconds_have_no_safe_cut_and_exact_final_pcm() -> TestResult {
    let mut dsp = AudioProcessor::new(AudioFormat {
        sample_rate: 16000,
        channels: 1,
    })?;
    let mut rule = ConfirmedPauses::default();
    for _ in 0..15000 {
        dsp.push_interleaved(&[0.1; 320])?;
        let (base, stable) = dsp.stable_samples();
        assert_eq!(rule.observe(base, stable), None);
    }
    let final_pcm = dsp.finish()?;
    assert_eq!(final_pcm.len(), MAX_SAMPLES);
    assert!(final_pcm.iter().all(|sample| *sample == 0.1));
    Ok(())
}

#[test]
fn unstable_or_already_sealed_ranges_are_rejected_without_losing_audio() -> TestResult {
    let mut dsp = AudioProcessor::new(AudioFormat {
        sample_rate: 44100,
        channels: 1,
    })?;
    dsp.push_interleaved(&[0.1; 4410])?;
    let (base, stable) = dsp.stable_samples();
    assert_eq!(base, 0);
    let stable_len = stable.len();
    assert!(dsp.seal_prefix(stable_len + 1).is_err());
    assert!(dsp.seal_prefix(0).is_err());
    assert_eq!(dsp.finish()?.len(), 1600);
    Ok(())
}
