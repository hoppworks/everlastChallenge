use super::*;

#[test]
fn bound_stop_timestamps_child_entry_before_lock_and_does_not_own_runtime()
-> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = MeasuredRuntime::spawn(
        InferenceService::with_decoder(Words)?,
        1,
        AudioFormat {
            sample_rate: 16000,
            channels: 1,
        },
    )?;
    wait_state(&runtime, |s| s.model_ready)?;
    let id = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
    let mut input = runtime.take_input(Duration::from_secs(2))?;
    input.port.publish(&[0.1; 3200]);
    let (entered, ingress) = bounded(1);
    runtime.stop_entry = Some(entered);
    let handle = runtime.stop_handle(id);
    let retained = handle.clone();
    let lock = runtime
        .runtime
        .client
        .shared
        .lock()
        .map_err(|_| "admission poisoned")?;
    let (send, done) = bounded(1);
    let child = thread::spawn(move || if send.send(handle.stop()).is_err() {});
    let early = ingress.recv_timeout(Duration::from_secs(2));
    let before_unlock = Instant::now();
    let blocked = done.recv_timeout(Duration::from_millis(20));
    drop(lock);
    let arrived = early?;
    assert!(matches!(
        blocked,
        Err(crossbeam_channel::RecvTimeoutError::Timeout)
    ));
    let returned = done.recv_timeout(Duration::from_secs(2))??;
    child.join().map_err(|_| "publisher panicked")?;
    assert_eq!(arrived, returned);
    assert!(arrived < before_unlock);
    wait_state(&runtime, |s| s.session.phase == Phase::Idle)?;
    assert_eq!(runtime.timing()?.t_stop, Some(arrived));
    // This consuming join succeeds while a cloned Stop endpoint remains alive.
    runtime.join()?;
    assert!(retained.stop().is_err());
    Ok(())
}

#[test]
fn stale_bound_stop_cannot_stamp_the_next_measured_session()
-> Result<(), Box<dyn std::error::Error>> {
    let runtime = MeasuredRuntime::spawn(
        InferenceService::with_decoder(Words)?,
        1,
        AudioFormat {
            sample_rate: 16000,
            channels: 1,
        },
    )?;
    wait_state(&runtime, |s| s.model_ready)?;
    let first = finish(&runtime)?.session.ok_or("first session missing")?;
    let old = runtime.stop_handle(first);
    let second = await_value(runtime.start(settings())?)?.map_err(std::io::Error::other)?;
    let mut input = runtime.take_input(Duration::from_secs(2))?;
    input.port.publish(&[0.1; 3200]);
    assert!(old.stop().is_err());
    assert_eq!(runtime.timing()?.t_stop, None);
    runtime.stop_handle(second).stop()?;
    wait_state(&runtime, |s| s.session.phase == Phase::Idle)?;
    assert_eq!(runtime.timing()?.session, Some(second));
    runtime.join()?;
    Ok(())
}
