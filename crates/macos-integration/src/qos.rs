//! Developer publisher QoS scope. No process/global policy or base QoS changes.

//! Uses public macOS `pthread/qos.h` override APIs (available since macOS 10.10).
//! The SDK specifies best-effort resource scheduling, not a realtime guarantee.
//! Requested base QoS is never changed, including QOS_CLASS_UNSPECIFIED.
use std::{ffi::c_void, marker::PhantomData, ptr::NonNull, rc::Rc};

#[link(name = "System")]
unsafe extern "C" {
    fn pthread_override_qos_class_start_np(
        thread: libc::pthread_t,
        class: libc::qos_class_t,
        relative_priority: libc::c_int,
    ) -> *mut c_void;
    fn pthread_override_qos_class_end_np(token: *mut c_void) -> libc::c_int;
}
struct Native;
impl Backend for Native {
    type Token = NonNull<c_void>;
    fn start(&self) -> Option<Self::Token> {
        // SAFETY: pthread_self is a live current thread; the public API accepts
        // USER_INTERACTIVE and relative priority zero. NULL has no promised errno.
        NonNull::new(unsafe {
            pthread_override_qos_class_start_np(
                libc::pthread_self(),
                libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE,
                0,
            )
        })
    }
    fn end(&self, token: Self::Token) -> Result<(), QosError> {
        // SAFETY: Scope owns exactly this start result and consumes it once.
        // The SDK deallocates the token on end; an error must not trigger reuse.
        let code = unsafe { pthread_override_qos_class_end_np(token.as_ptr()) };
        if code == 0 {
            Ok(())
        } else {
            Err(QosError::Finish(code))
        }
    }
}

/// Same-thread publisher override. Explicit finish reports cleanup failure;
/// Drop attempts cleanup only when finish has not consumed the owned token.
/// Start after worker creation; finish before observer/report work.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<macos_integration::qos::PublisherQos>();
/// ```
/// ```compile_fail
/// fn require_sync<T: Sync>() {}
/// require_sync::<macos_integration::qos::PublisherQos>();
/// ```
pub struct PublisherQos(Scope<Native>);
impl PublisherQos {
    pub fn enter() -> Result<Self, QosError> {
        Scope::enter_with(Native).map(Self)
    }
    pub fn finish(self) -> Result<(), QosError> {
        self.0.finish()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum QosError {
    #[error("Publisher QoS override could not be started")]
    Unavailable,
    #[error("Publisher QoS override cleanup was not confirmed (code {0})")]
    Finish(i32),
}

trait Backend {
    type Token;
    fn start(&self) -> Option<Self::Token>;
    fn end(&self, token: Self::Token) -> Result<(), QosError>;
}
struct Scope<B: Backend> {
    backend: B,
    token: Option<B::Token>,
    _same_thread: PhantomData<Rc<()>>,
}
impl<B: Backend> Scope<B> {
    fn enter_with(backend: B) -> Result<Self, QosError> {
        let token = backend.start().ok_or(QosError::Unavailable)?;
        Ok(Self {
            backend,
            token: Some(token),
            _same_thread: PhantomData,
        })
    }
    fn finish(mut self) -> Result<(), QosError> {
        match self.token.take() {
            Some(token) => self.backend.end(token),
            None => Ok(()),
        }
    }
}
impl<B: Backend> Drop for Scope<B> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            let _cleanup = self.backend.end(token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    struct State {
        starts: Cell<u32>,
        ends: Cell<u32>,
        unavailable: Cell<bool>,
        end_error: Cell<Option<i32>>,
        // The backend exposes only override operations, no base-QoS setter.
        requested_base: Cell<u32>,
    }
    struct Fake(Rc<State>);
    impl Backend for Fake {
        type Token = u64;
        fn start(&self) -> Option<u64> {
            self.0.starts.set(self.0.starts.get() + 1);
            (!self.0.unavailable.get()).then_some(97)
        }
        fn end(&self, token: u64) -> Result<(), QosError> {
            assert_eq!(token, 97);
            self.0.ends.set(self.0.ends.get() + 1);
            self.0
                .end_error
                .get()
                .map_or(Ok(()), |code| Err(QosError::Finish(code)))
        }
    }
    #[test]
    fn unspecified_base_is_preserved_and_explicit_finish_ends_exactly_one_owned_token()
    -> Result<(), QosError> {
        let state = Rc::new(State::default());
        let scope = Scope::enter_with(Fake(Rc::clone(&state)))?;
        assert_eq!(state.starts.get(), 1);
        assert_eq!(state.ends.get(), 0);
        assert_eq!(state.requested_base.get(), 0);
        scope.finish()?;
        assert_eq!(state.ends.get(), 1);
        assert_eq!(state.requested_base.get(), 0);
        Ok(())
    }
    #[test]
    fn failed_enter_never_owns_or_ends_a_token() {
        let state = Rc::new(State::default());
        state.unavailable.set(true);
        assert!(matches!(
            Scope::enter_with(Fake(Rc::clone(&state))),
            Err(QosError::Unavailable)
        ));
        assert_eq!(state.starts.get(), 1);
        assert_eq!(state.ends.get(), 0);
    }
    #[test]
    fn failed_finish_is_explicit_and_does_not_end_the_same_token_twice() -> Result<(), QosError> {
        let state = Rc::new(State::default());
        state.end_error.set(Some(22));
        let scope = Scope::enter_with(Fake(Rc::clone(&state)))?;
        assert_eq!(scope.finish(), Err(QosError::Finish(22)));
        assert_eq!(state.ends.get(), 1);
        Ok(())
    }
    #[test]
    fn drop_and_panic_unwind_each_end_their_owned_token() -> Result<(), QosError> {
        let state = Rc::new(State::default());
        drop(Scope::enter_with(Fake(Rc::clone(&state)))?);
        assert_eq!(state.ends.get(), 1);
        let scope = Scope::enter_with(Fake(Rc::clone(&state)))?;
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _scope = scope;
            std::panic::resume_unwind(Box::new("controlled Rust unwind"));
        }));
        assert!(unwound.is_err());
        assert_eq!(state.ends.get(), 2);
        Ok(())
    }
}
