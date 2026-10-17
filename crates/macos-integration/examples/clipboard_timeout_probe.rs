//! Controlled helper stall: no clipboard calls or persistent data.
use macos_integration::{
    clipboard::{ClipboardError, capture_bounded},
    delivery::DeliveryToken,
};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--everlast-clipboard-snapshot") {
        if std::env::args().nth(2).as_deref() == Some("unavailable") {
            return Err("simulated unavailable pasteboard items".into());
        }
        std::thread::sleep(Duration::from_secs(30));
        return Ok(());
    }
    let token = DeliveryToken::new(NonZeroU64::new(1).ok_or("invalid id")?);
    if capture_bounded(&std::env::current_exe()?, "unavailable", &token).is_ok() {
        return Err("failed helper was accepted as an empty clipboard".into());
    }
    let started = Instant::now();
    if !matches!(
        capture_bounded(&std::env::current_exe()?, "unused", &token),
        Err(ClipboardError::Timeout)
    ) || started.elapsed() >= Duration::from_secs(3)
    {
        return Err("stalled helper was not bounded".into());
    }
    let cancel = token.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        cancel.cancel()
    });
    let started = Instant::now();
    let result = capture_bounded(&std::env::current_exe()?, "unused", &token);
    canceller.join().map_err(|_| "canceller failed")?;
    if !matches!(result, Err(ClipboardError::Cancelled))
        || started.elapsed() >= Duration::from_secs(1)
    {
        return Err("stalled helper did not cancel promptly".into());
    }
    println!("Stalled helper: deadline and cancellation passed, child processes reaped");
    Ok(())
}
