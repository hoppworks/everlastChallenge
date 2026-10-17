//! Developer lifecycle acceptance using a verified model directory, without recording or pasting.
use crossbeam_channel::{TryRecvError, bounded};
use everlast_dictation::{
    application::AppSupervisor, model::REFERENCE, native_service::NativeService,
};
use macos_integration::clipboard::snapshot_helper_main;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = snapshot_helper_main() {
        return result.map_err(Into::into);
    }
    let directory = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("model directory required")?,
    );
    let fail_native = std::env::args().any(|argument| argument == "--fail-native");
    let helper = std::env::current_exe()?;
    let (queue, main_requests) = bounded(1);
    let supervisor = AppSupervisor::spawn(directory, REFERENCE, move |endpoint| {
        if fail_native {
            return Err("fixture-native-start-failed".into());
        }
        NativeService::spawn(endpoint, helper, move |request| {
            queue.try_send(request).map_err(|error| error.to_string())
        })
    })?;
    let client = supervisor.client();
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut phase = None;
    loop {
        while let Ok(request) = main_requests.try_recv() {
            request.execute();
        }
        let state = client.snapshot()?;
        if phase != Some(state.setup.phase) {
            println!("Setup: {:?}", state.setup.phase);
            phase = Some(state.setup.phase);
        }
        if !state.running {
            if !fail_native
                || !state
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("fixture-native-start-failed"))
            {
                return Err(format!("unexpected application exit: {:?}", state.error).into());
            }
            break;
        }
        if state.runtime_available {
            if fail_native {
                return Err("runtime was published despite native start failure".into());
            }
            let runtime = client.runtime()?.ok_or("published runtime missing")?;
            let runtime = runtime.snapshot()?;
            if !runtime.model_ready {
                return Err("runtime published before model ready".into());
            }
            println!(
                "Runtime ready; session={:?}; no recording or paste requested",
                runtime.session.phase
            );
            break;
        }
        if matches!(
            state.setup.phase,
            everlast_dictation::setup::Phase::Failed | everlast_dictation::setup::Phase::Missing
        ) && state.revision > 1
        {
            return Err(format!("model unavailable: {:?}", state.setup.failure).into());
        }
        if Instant::now() >= deadline {
            return Err("application initialization timed out".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    client.shutdown()?;
    let (finished, result) = bounded(1);
    let join = thread::spawn(move || {
        if finished.send(supervisor.join()).is_err() { /* probe stopped listening */ }
    });
    let outcome = loop {
        while let Ok(request) = main_requests.try_recv() {
            request.execute();
        }
        match result.try_recv() {
            Ok(outcome) => break outcome,
            Err(TryRecvError::Disconnected) => return Err("application join result missing".into()),
            Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(10)),
        }
    };
    join.join().map_err(|_| "join thread panicked")?;
    if fail_native {
        if outcome.is_ok() {
            return Err("injected native failure was lost".into());
        }
        println!(
            "Native start failure remained visible; initialized runtime and model owner cleaned up"
        );
    } else {
        outcome?;
        println!("Application stopped cleanly; model and native owners joined");
    }
    Ok(())
}
