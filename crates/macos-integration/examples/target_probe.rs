//! Read-only developer check: no TCC prompt, activation, text or clipboard IO.
use macos_integration::{
    delivery::DeliveryToken,
    target::{
        accessibility_trusted, frontmost_process, secure_input_active,
        worker::{TargetEvent, TargetWorker},
    },
};
use objc2::MainThreadMarker;
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

fn response(worker: &TargetWorker) -> Result<TargetEvent, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(event) = worker.next_event()? {
            return Ok(event);
        }
        if Instant::now() >= deadline {
            return Err("native response timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_thread = MainThreadMarker::new().ok_or("not on the main thread")?;
    println!(
        "Accessibility trusted: {}; secure input active: {}",
        accessibility_trusted(),
        secure_input_active()
    );
    let process = frontmost_process(main_thread)?;
    let worker = TargetWorker::start()?;
    let session = NonZeroU64::new(1).ok_or("invalid session")?;
    worker.capture(session, process, DeliveryToken::new(session))?;
    match response(&worker)? {
        TargetEvent::Captured {
            result: Err(error), ..
        } => println!("Target held by worker: {error}"),
        TargetEvent::Captured { result: Ok(()), .. } => {
            worker.approve(session, session, frontmost_process(main_thread)?)?;
            match response(&worker)? {
                TargetEvent::Approved {
                    result: Ok(receipt),
                    ..
                } => {
                    receipt.validate(session, frontmost_process(main_thread)?)?;
                    println!("Worker captured and revalidated the initial field");
                }
                TargetEvent::Approved {
                    result: Err(error), ..
                } => println!("Target held by worker: {error}"),
                _ => return Err("unexpected native event".into()),
            }
        }
        _ => return Err("unexpected native event".into()),
    }
    worker.shutdown()?;
    Ok(())
}
