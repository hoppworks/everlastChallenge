//! Explicit fixture-only smoke test; no transcript is written to diagnostics.
use everlast_dictation::{
    model::REFERENCE,
    transcription::{Engine, Language},
};
use std::{path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() != 3 {
        return Err("usage: transcribe_probe <reference-model-path> <jfk-wav-path>".into());
    }
    let mut reader = hound::WavReader::open(&arguments[2])?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != 16_000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err("probe fixture must be PCM16 mono at 16000 Hz".into());
    }
    let samples: Result<Vec<f32>, _> = reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| f32::from(value) / 32_768.0))
        .collect();
    let samples = samples?;
    let started = Instant::now();
    let mut engine = Engine::load(Path::new(&arguments[1]), REFERENCE)?;
    println!("model loaded in {:?}", started.elapsed());
    for run in 1..=2 {
        let started = Instant::now();
        let text = engine.transcribe(&samples, Language::English)?;
        if !text
            .to_lowercase()
            .contains("ask not what your country can do for you")
        {
            return Err("known fixture phrase was not recognized".into());
        }
        println!(
            "fixture run {run}: recognized known phrase, {} bytes, {:?}",
            text.len(),
            started.elapsed()
        );
    }
    Ok(())
}
