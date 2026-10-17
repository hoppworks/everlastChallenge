//! Native feedback without focus, window, input or notification actions.
use objc2::MainThreadMarker;
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FeedbackError {
    #[error("Native feedback requires the main thread")]
    MainThread,
}
/// Request one system alert sound. macOS/user audio settings control audibility;
/// success means the request was issued, never that a person heard it.
/// Caller must additionally hold Runtime's idle capture-admission guard.
pub fn held_tone() -> Result<(), FeedbackError> {
    let _main = MainThreadMarker::new().ok_or(FeedbackError::MainThread)?;
    objc2_app_kit::NSBeep();
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn worker_is_rejected_before_requesting_a_sound() -> Result<(), String> {
        let result = std::thread::spawn(super::held_tone)
            .join()
            .map_err(|_| "worker panicked")?;
        assert_eq!(result, Err(super::FeedbackError::MainThread));
        Ok(())
    }
}
