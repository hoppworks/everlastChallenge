//! Explicit fixture-only smoke test; no transcript is written to diagnostics.
use everlast_dictation::{
    inference_service::{InferenceService, Job, WorkToken},
    model::REFERENCE,
    transcription::Language,
};
use std::{
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

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
    let samples = Arc::new(samples?);
    let started = Instant::now();
    let service = InferenceService::load(PathBuf::from(&arguments[1]), REFERENCE)?;
    while !service.ready() {
        if service.is_finished() {
            service.shutdown()?;
            return Err("model worker stopped before readiness".into());
        }
        if started.elapsed() > Duration::from_secs(120) {
            return Err("model readiness observation timed out".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
    println!("model worker ready in {:?}", started.elapsed());
    for run in 1..=2 {
        let started = Instant::now();
        service.submit(Job {
            session: run,
            generation: 1,
            job_id: run,
            sample_range: 0..samples.len(),
            audio: Arc::clone(&samples),
            language: Language::English,
            token: WorkToken::default(),
        })?;
        let completed = loop {
            if let Some(result) = service.take_result()? {
                break result;
            }
            if service.is_finished() {
                service.shutdown()?;
                return Err("inference worker stopped before result".into());
            }
            if started.elapsed() > Duration::from_secs(120) {
                return Err("inference observation timed out".into());
            }
            thread::sleep(Duration::from_millis(1));
        };
        if completed.session != run
            || completed.generation != 1
            || completed.job_id != run
            || completed.sample_range != (0..samples.len())
        {
            return Err("inference worker returned mismatched identity".into());
        }
        let text = completed.result?;
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
    service.shutdown()?;
    Ok(())
}
