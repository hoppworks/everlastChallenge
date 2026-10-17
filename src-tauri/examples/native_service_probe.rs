//! Read-only target capture through the full native service. Never sends a key event.
use crossbeam_channel::{TryRecvError, bounded};
use everlast_dictation::{
    native_service::NativeService,
    runtime::{NativeCommand, NativeEndpoint, NativeResult},
};
use macos_integration::{
    clipboard::snapshot_helper_main,
    delivery::DeliveryToken,
    main_delivery::{self, Command, Reply},
};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = snapshot_helper_main() {
        return result.map_err(Into::into);
    }
    let Reply::Environment(environment) = main_delivery::dispatch(Command::Environment)? else {
        return Err("environment missing".into());
    };
    println!(
        "Accessibility trusted={}, Secure Input={}",
        environment.accessibility_trusted, environment.secure_input
    );
    let (send, requests) = bounded(8);
    let (responses, receive) = bounded(8);
    let (schedule, main_requests) = bounded(1);
    let service = NativeService::spawn(
        NativeEndpoint {
            requests,
            responses,
        },
        std::env::current_exe()?,
        move |request| {
            schedule
                .try_send(request)
                .map_err(|error| error.to_string())
        },
    )?;
    let session = NonZeroU64::new(1).ok_or("invalid session")?;
    let token = DeliveryToken::new(session);
    send.send(NativeCommand::Capture {
        session,
        process: Some(environment.process),
        token: token.clone(),
    })?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let result = loop {
        if let Ok(request) = main_requests.try_recv() {
            request.execute();
        }
        match receive.try_recv() {
            Ok(NativeResult::Captured { session: 1, result }) => break result,
            Ok(_) => return Err("unexpected native response".into()),
            Err(TryRecvError::Disconnected) => return Err("native service disconnected".into()),
            Err(TryRecvError::Empty) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(TryRecvError::Empty) => return Err("capture deadline expired".into()),
        }
    };
    token.cancel();
    service.join()?;
    match result {
        Ok(()) => println!("Frozen target verified; no clipboard data read and no event sent"),
        Err(reason) => println!("Target held: {reason}; no clipboard data read and no event sent"),
    }
    Ok(())
}
