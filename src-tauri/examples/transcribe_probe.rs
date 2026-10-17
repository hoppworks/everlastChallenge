//! Explicit developer file replay; no transcript is written to diagnostics.
#[path = "transcribe_probe/input.rs"]
mod input;
use everlast_dictation::{
    inference_service::{InferenceService, Job, WorkToken},
    model::REFERENCE,
};
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args_os()
        .skip(1)
        .take(input::MAX_ARGUMENTS + 1)
        .collect::<Vec<_>>();
    let options = input::Options::parse(&arguments)?;
    let samples = Arc::new(input::load_audio(&options.wav)?);
    let reference = options
        .reference
        .as_deref()
        .map(input::load_reference)
        .transpose()?;
    let started = Instant::now();
    let service = InferenceService::load(options.model, REFERENCE)?;
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
    for run in 1..=options.repeats {
        let job_id = u64::from(run);
        let started = Instant::now();
        service.submit(Job {
            session: job_id,
            generation: 1,
            job_id,
            sample_range: 0..samples.len(),
            audio: Arc::clone(&samples),
            language: options.language,
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
        if completed.session != job_id
            || completed.generation != 1
            || completed.job_id != job_id
            || completed.sample_range != (0..samples.len())
        {
            return Err("inference worker returned mismatched identity".into());
        }
        let text = completed.result?;
        if options.legacy {
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
        } else {
            let matches = reference
                .as_ref()
                .map(|expected| input::reference_matches(&text, expected));
            println!(
                "replay run {run}: {} bytes, {:?}, reference_match={}",
                text.len(),
                started.elapsed(),
                matches.map_or("not_requested", |value| if value {
                    "true"
                } else {
                    "false"
                })
            );
            if matches == Some(false) {
                return Err("replay transcript did not match the supplied reference".into());
            }
        }
    }
    service.shutdown()?;
    Ok(())
}
