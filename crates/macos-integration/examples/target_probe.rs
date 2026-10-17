//! Read-only developer check: no TCC prompt, activation, text or clipboard IO.
use macos_integration::target::{
    Target, TargetError, accessibility_trusted, frontmost_process, secure_input_active,
};
use objc2::MainThreadMarker;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_thread = MainThreadMarker::new().ok_or("not on the main thread")?;
    println!(
        "Accessibility trusted: {}; secure input active: {}",
        accessibility_trusted(),
        secure_input_active()
    );
    if !accessibility_trusted() {
        println!("Target inspection skipped: permission missing; no prompt requested");
        return Ok(());
    }
    let process = frontmost_process(main_thread)?;
    let (captured, capture_ready) = std::sync::mpsc::sync_channel(0);
    let (send_frontmost, receive_frontmost) = std::sync::mpsc::sync_channel(0);
    let worker = std::thread::spawn(move || {
        let session = std::num::NonZeroU64::new(1).ok_or(TargetError::Process)?;
        let target = Target::capture(session, process)?;
        captured.send(()).map_err(|_| TargetError::Process)?;
        let frontmost = receive_frontmost
            .recv()
            .map_err(|_| TargetError::Process)??;
        target.verify(frontmost)
    });
    if capture_ready.recv().is_ok() {
        send_frontmost.send(frontmost_process(main_thread))?;
    }
    let result = worker.join().map_err(|_| "target worker failed")?;
    match result {
        Ok(()) => println!("Initial field captured and immediately revalidated"),
        Err(error) => println!("Target held: {error}"),
    }
    Ok(())
}
