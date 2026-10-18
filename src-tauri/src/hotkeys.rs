//! Owned physical shortcuts. Registry operations belong on a serial background owner;
//! the Tauri event loop must remain available because the plugin waits for the main thread.
//! Successful user callbacks execute on the main thread before any UI is shown.
//! Registration success does not prove printable-character suppression or Secure Input behavior.
//! The local global-hotkey 0.8.0 patch preserves numeric Carbon OSStatus and retains native
//! ownership after failed removal. Controlled tests do not establish actual OS-error recovery;
//! installed-app acceptance remains required.
use crate::settings::{Hotkey, PhysicalKey};
use macos_integration::{
    main_delivery::{self, Command, Reply},
    target::ProcessIdentity,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Zero-sized without measurement; timestamped before host gate/dispatch otherwise.
#[derive(Clone, Copy)]
pub struct NativeOrigin {
    #[cfg(feature = "measurement")]
    pub received_at: Option<std::time::Instant>,
}
impl NativeOrigin {
    fn now(_observe: bool) -> Self {
        Self {
            #[cfg(feature = "measurement")]
            received_at: _observe.then(std::time::Instant::now),
        }
    }
}
pub enum Event {
    Toggle {
        process: Option<ProcessIdentity>,
        origin: NativeOrigin,
    },
    Cancel {
        session: u64,
    },
}
#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("A shortcut needs Option, Command or Control")]
    Invalid,
    #[error("Hotkey registry failed: {0}")]
    Registry(String),
    #[error("Hotkey owner has shut down")]
    Closed,
    #[error("Cancellation shortcut needs a positive session identifier")]
    InvalidSession,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Toggle,
    Cancel(u64),
}
#[derive(Default)]
struct Gate {
    enabled: AtomicBool,
    pressed: Arc<Mutex<bool>>,
    pending: AtomicBool,
}
struct Pending(Arc<Gate>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.pending.store(false, Ordering::Release);
    }
}
impl Gate {
    fn reserve(self: &Arc<Self>) -> Option<Pending> {
        self.pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Pending(Arc::clone(self)))
    }
    fn accept(&self, state: ShortcutState) -> bool {
        let Ok(mut pressed) = self.pressed.lock() else {
            return false;
        };
        match state {
            ShortcutState::Pressed => {
                if !self.enabled.load(Ordering::Acquire) {
                    return false;
                }
                let first = !*pressed;
                *pressed = true;
                first
            }
            ShortcutState::Released => {
                *pressed = false;
                false
            }
        }
    }
}
type Handler = Arc<dyn Fn(ShortcutState, NativeOrigin) + Send + Sync>;
type Dispatch = Arc<dyn Fn(Kind, Arc<Gate>, NativeOrigin) + Send + Sync>;
trait Registry: Send {
    fn escape_pressed(&self) -> bool;
    fn register(&mut self, key: Shortcut, handler: Handler) -> Result<(), String>;
    fn unregister(&mut self, key: Shortcut) -> Result<(), String>;
}
struct NativeRegistry(tauri::AppHandle, bool);
impl Registry for NativeRegistry {
    fn escape_pressed(&self) -> bool {
        macos_integration::keyboard::escape_key_pressed()
    }
    fn register(&mut self, key: Shortcut, handler: Handler) -> Result<(), String> {
        let observe = self.1;
        self.0
            .global_shortcut()
            .on_shortcut(key, move |_, _, event| {
                let origin = NativeOrigin::now(observe);
                handler(event.state, origin);
            })
            .map_err(|error| error.to_string())
    }
    fn unregister(&mut self, key: Shortcut) -> Result<(), String> {
        self.0
            .global_shortcut()
            .unregister(key)
            .map_err(|error| error.to_string())
    }
}
struct Binding {
    key: Shortcut,
    gate: Arc<Gate>,
    kind: Kind,
}
pub struct Hotkeys {
    registry: Box<dyn Registry>,
    dispatch: Dispatch,
    toggle: Option<Binding>,
    configuration: Hotkey,
    cancel: Option<Binding>,
    orphan: Option<Binding>,
    errors: Arc<Mutex<Option<String>>>,
    cancel_pressed: Arc<Mutex<bool>>,
    closed: bool,
}
impl Hotkeys {
    /// Call off the GUI thread, after installing the global-shortcut plugin.
    /// Registration failure retains the plugin's error text, including Carbon OSStatus
    /// from the local backend patch. No synthetic keys are sent.
    pub fn install(
        app: tauri::AppHandle,
        key: Hotkey,
        callback: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, HotkeyError> {
        Self::install_inner(app, key, callback, false)
    }
    /// Explicit developer opt-in; normal installation retains no origin clocks.
    #[cfg(feature = "measurement")]
    pub fn install_observed(
        app: tauri::AppHandle,
        key: Hotkey,
        callback: impl Fn(Event) + Send + Sync + 'static,
        enabled: bool,
    ) -> Result<Self, HotkeyError> {
        Self::install_inner(app, key, callback, enabled)
    }
    fn install_inner(
        app: tauri::AppHandle,
        key: Hotkey,
        callback: impl Fn(Event) + Send + Sync + 'static,
        observe: bool,
    ) -> Result<Self, HotkeyError> {
        let errors = Arc::new(Mutex::new(None));
        let failures = Arc::clone(&errors);
        let callback = Arc::new(callback);
        let dispatcher = app.clone();
        let dispatch: Dispatch = Arc::new(move |kind, gate, _origin| {
            let Some(pending) = gate.reserve() else {
                record_error(
                    &failures,
                    "Hotkey main-thread request still outstanding".into(),
                );
                return;
            };
            let callback = Arc::clone(&callback);
            let failures_main = Arc::clone(&failures);
            if let Err(error) = dispatcher.run_on_main_thread(move || {
                let _pending = pending;
                if !gate.enabled.load(Ordering::Acquire) {
                    return;
                }
                match kind {
                    Kind::Toggle => {
                        let process = match main_delivery::dispatch(Command::Environment) {
                            Ok(Reply::Environment(environment)) => Some(environment.process),
                            Ok(_) => {
                                record_error(
                                    &failures_main,
                                    "Unexpected native environment reply".into(),
                                );
                                None
                            }
                            Err(error) => {
                                record_error(&failures_main, error.to_string());
                                None
                            }
                        };
                        callback(Event::Toggle {
                            process,
                            origin: _origin,
                        });
                    }
                    Kind::Cancel(session) => callback(Event::Cancel { session }),
                }
            }) {
                record_error(&failures, error.to_string());
            }
        });
        Self::with_registry(
            Box::new(NativeRegistry(app, observe)),
            key,
            dispatch,
            errors,
        )
    }
    fn with_registry(
        registry: Box<dyn Registry>,
        key: Hotkey,
        dispatch: Dispatch,
        errors: Arc<Mutex<Option<String>>>,
    ) -> Result<Self, HotkeyError> {
        let mut owner = Self {
            registry,
            dispatch,
            toggle: None,
            configuration: key.clone(),
            cancel: None,
            orphan: None,
            errors,
            cancel_pressed: Arc::new(Mutex::new(false)),
            closed: false,
        };
        let binding = owner.register(shortcut(&key)?, Kind::Toggle)?;
        binding.gate.enabled.store(true, Ordering::Release);
        owner.toggle = Some(binding);
        Ok(owner)
    }
    fn register(&mut self, key: Shortcut, kind: Kind) -> Result<Binding, HotkeyError> {
        let gate = Arc::new(Gate {
            // The physical Escape key may stay held while its session binding
            // changes. Rebinding must not turn OS autorepeat into a fresh press.
            pressed: if matches!(kind, Kind::Cancel(_)) {
                Arc::clone(&self.cancel_pressed)
            } else {
                Arc::new(Mutex::new(false))
            },
            ..Gate::default()
        });
        let event_gate = Arc::clone(&gate);
        let dispatch = Arc::clone(&self.dispatch);
        self.registry
            .register(
                key,
                Arc::new(move |state, origin| {
                    if event_gate.accept(state) {
                        dispatch(kind, Arc::clone(&event_gate), origin);
                    }
                }),
            )
            .map_err(HotkeyError::Registry)?;
        Ok(Binding { key, gate, kind })
    }
    /// Last successfully registered configuration, only while its owned toggle
    /// remains active. This reports our registration ownership, not a new OS probe.
    pub fn current_binding(&self) -> Option<Hotkey> {
        let binding = self.toggle.as_ref()?;
        if self.closed
            || !binding.gate.enabled.load(Ordering::Acquire)
            || binding.gate.pressed.is_poisoned()
            || binding.kind != Kind::Toggle
            || shortcut(&self.configuration).ok() != Some(binding.key)
        {
            return None;
        }
        Some(self.configuration.clone())
    }
    /// Capture infrastructure errors without delivering callbacks from a non-main thread.
    pub fn take_error(&self) -> Result<Option<String>, HotkeyError> {
        self.errors
            .lock()
            .map(|mut error| error.take())
            .map_err(|_| HotkeyError::Registry("Hotkey error lock poisoned".into()))
    }
    /// Escape is globally registered only for the supplied active session.
    /// Changing its ID revokes pending callbacks and creates a fresh binding;
    /// callbacks carry that immutable ID rather than looking up a newer session.
    /// Escape's physical state is sampled on activation, including releases that
    /// occurred while its shortcut was absent; repeats of a held key stay suppressed.
    /// Call from the same background owner as install/reconfigure/shutdown.
    pub fn enable_cancel(&mut self, session: Option<u64>) -> Result<(), HotkeyError> {
        if self.closed {
            return Err(HotkeyError::Closed);
        }
        if session == Some(0) {
            return Err(HotkeyError::InvalidSession);
        }
        if let Some(binding) = &self.cancel {
            if session.is_some_and(|id| binding.kind == Kind::Cancel(id))
                && binding.gate.enabled.load(Ordering::Acquire)
            {
                return Ok(());
            }
            // Revoke queued callbacks before attempting native removal. Reusing
            // a live Escape registration for a different session would retarget
            // old key events to the new recording.
            binding.gate.enabled.store(false, Ordering::Release);
            self.registry
                .unregister(binding.key)
                .map_err(HotkeyError::Registry)?;
            self.cancel = None;
        }
        if let Some(session) = session {
            let binding =
                self.register(Shortcut::new(None, Code::Escape), Kind::Cancel(session))?;
            self.cancel = Some(binding);
            let gate = &self.cancel.as_ref().ok_or(HotkeyError::Closed)?.gate;
            // Registration can miss releases during an idle gap. Sample the
            // physical state while excluding callback updates, then activate
            // the new session gate before allowing callbacks to proceed.
            let mut pressed = gate
                .pressed
                .lock()
                .map_err(|_| HotkeyError::Registry("Escape latch poisoned".into()))?;
            *pressed = self.registry.escape_pressed();
            gate.enabled.store(true, Ordering::Release);
        }
        Ok(())
    }
    /// Transactional rebinding: retain the old working shortcut if registration or removal fails.
    /// A failed rollback is retained as one disabled owned binding for a later cleanup retry.
    pub fn reconfigure(&mut self, key: Hotkey) -> Result<(), HotkeyError> {
        if self.closed {
            return Err(HotkeyError::Closed);
        }
        if let Some(binding) = &self.orphan {
            self.registry
                .unregister(binding.key)
                .map_err(HotkeyError::Registry)?;
            self.orphan = None;
        }
        let next = shortcut(&key)?;
        if self
            .toggle
            .as_ref()
            .is_some_and(|binding| binding.key == next)
        {
            return Ok(());
        }
        let binding = self.register(next, Kind::Toggle)?;
        if let Some(previous) = &self.toggle {
            if let Err(error) = self.registry.unregister(previous.key) {
                let rollback = self.registry.unregister(binding.key);
                if let Err(rollback_error) = rollback {
                    self.orphan = Some(binding);
                    return Err(HotkeyError::Registry(format!(
                        "{error}; rollback failed: {rollback_error}"
                    )));
                }
                return Err(HotkeyError::Registry(error));
            }
            previous.gate.enabled.store(false, Ordering::Release);
        }
        binding.gate.enabled.store(true, Ordering::Release);
        self.toggle = Some(binding);
        self.configuration = key;
        Ok(())
    }
    /// Disables callbacks immediately, unregistering only keys owned by this value.
    /// The event loop must remain running. Failed removals are retained for a retry.
    pub fn shutdown(&mut self) -> Result<(), HotkeyError> {
        self.closed = true;
        let mut errors = Vec::new();
        for slot in [&mut self.toggle, &mut self.cancel, &mut self.orphan] {
            if let Some(binding) = slot {
                binding.gate.enabled.store(false, Ordering::Release);
                match self.registry.unregister(binding.key) {
                    Ok(()) => *slot = None,
                    Err(error) => errors.push(error),
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(HotkeyError::Registry(errors.join("; ")))
        }
    }
}
impl Drop for Hotkeys {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Owned shortcut cleanup failed: {error}");
        }
    }
}
fn record_error(errors: &Mutex<Option<String>>, error: String) {
    if let Ok(mut stored) = errors.lock() {
        *stored = Some(error);
    }
}
fn shortcut(key: &Hotkey) -> Result<Shortcut, HotkeyError> {
    if !(key.option || key.command || key.control) {
        return Err(HotkeyError::Invalid);
    }
    let mut modifiers = Modifiers::empty();
    if key.option {
        modifiers |= Modifiers::ALT;
    }
    if key.command {
        modifiers |= Modifiers::SUPER;
    }
    if key.control {
        modifiers |= Modifiers::CONTROL;
    }
    if key.shift {
        modifiers |= Modifiers::SHIFT;
    }
    let code = match key.key {
        PhysicalKey::A => Code::KeyA,
        PhysicalKey::B => Code::KeyB,
        PhysicalKey::C => Code::KeyC,
        PhysicalKey::D => Code::KeyD,
        PhysicalKey::E => Code::KeyE,
        PhysicalKey::F => Code::KeyF,
        PhysicalKey::G => Code::KeyG,
        PhysicalKey::H => Code::KeyH,
        PhysicalKey::I => Code::KeyI,
        PhysicalKey::J => Code::KeyJ,
        PhysicalKey::K => Code::KeyK,
        PhysicalKey::L => Code::KeyL,
        PhysicalKey::M => Code::KeyM,
        PhysicalKey::N => Code::KeyN,
        PhysicalKey::O => Code::KeyO,
        PhysicalKey::P => Code::KeyP,
        PhysicalKey::Q => Code::KeyQ,
        PhysicalKey::R => Code::KeyR,
        PhysicalKey::S => Code::KeyS,
        PhysicalKey::T => Code::KeyT,
        PhysicalKey::U => Code::KeyU,
        PhysicalKey::V => Code::KeyV,
        PhysicalKey::W => Code::KeyW,
        PhysicalKey::X => Code::KeyX,
        PhysicalKey::Y => Code::KeyY,
        PhysicalKey::Z => Code::KeyZ,
    };
    Ok(Shortcut::new(Some(modifiers), code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::atomic::AtomicUsize};
    #[derive(Default)]
    struct FakeState {
        handlers: HashMap<u32, Handler>,
        removals: Vec<u32>,
        fail_remove: Vec<u32>,
        fail_register: bool,
        escape_pressed: bool,
    }
    struct FakeRegistry(Arc<Mutex<FakeState>>);
    impl Registry for FakeRegistry {
        fn escape_pressed(&self) -> bool {
            self.0
                .lock()
                .map(|state| state.escape_pressed)
                .unwrap_or(true)
        }
        fn register(&mut self, key: Shortcut, handler: Handler) -> Result<(), String> {
            let mut state = self.0.lock().map_err(|_| "fake registry poisoned")?;
            if state.fail_register {
                return Err("registration conflict".into());
            }
            state.handlers.insert(key.id(), handler);
            Ok(())
        }
        fn unregister(&mut self, key: Shortcut) -> Result<(), String> {
            let mut state = self.0.lock().map_err(|_| "fake registry poisoned")?;
            state.removals.push(key.id());
            if state.fail_remove.contains(&key.id()) {
                return Err("unregister failed".into());
            }
            state.handlers.remove(&key.id());
            Ok(())
        }
    }
    struct Fixture {
        owner: Hotkeys,
        state: Arc<Mutex<FakeState>>,
        toggles: Arc<AtomicUsize>,
        cancels: Arc<AtomicUsize>,
        cancelled_sessions: Arc<Mutex<Vec<u64>>>,
    }
    impl Fixture {
        fn new() -> Result<Self, HotkeyError> {
            let state = Arc::new(Mutex::new(FakeState::default()));
            let toggles = Arc::new(AtomicUsize::new(0));
            let cancels = Arc::new(AtomicUsize::new(0));
            let cancelled_sessions = Arc::new(Mutex::new(Vec::new()));
            let recorded_sessions = Arc::clone(&cancelled_sessions);
            let (toggle_actions, cancel_actions) = (Arc::clone(&toggles), Arc::clone(&cancels));
            let dispatch: Dispatch = Arc::new(move |kind, gate, _origin| {
                if gate.enabled.load(Ordering::Acquire) {
                    match kind {
                        Kind::Toggle => {
                            toggle_actions.fetch_add(1, Ordering::AcqRel);
                        }
                        Kind::Cancel(session) => {
                            cancel_actions.fetch_add(1, Ordering::AcqRel);
                            if let Ok(mut sessions) = recorded_sessions.lock() {
                                sessions.push(session);
                            }
                        }
                    }
                }
            });
            let owner = Hotkeys::with_registry(
                Box::new(FakeRegistry(Arc::clone(&state))),
                crate::settings::Settings::default().hotkey,
                dispatch,
                Arc::new(Mutex::new(None)),
            )?;
            Ok(Self {
                owner,
                state,
                toggles,
                cancels,
                cancelled_sessions,
            })
        }
        fn handler(&self, key: Shortcut) -> Result<Handler, Box<dyn std::error::Error>> {
            let handler = self
                .state
                .lock()
                .map_err(|_| "fake registry poisoned")?
                .handlers
                .get(&key.id())
                .cloned()
                .ok_or("handler missing")?;
            let state = Arc::clone(&self.state);
            Ok(Arc::new(move |event, origin| {
                if key == Shortcut::new(None, Code::Escape)
                    && let Ok(mut state) = state.lock()
                {
                    state.escape_pressed = matches!(event, ShortcutState::Pressed);
                }
                handler(event, origin);
            }))
        }
    }
    #[test]
    fn refreshing_active_cancel_does_not_reset_held_key() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut fixture = Fixture::new()?;
        let escape = Shortcut::new(None, Code::Escape);
        assert!(fixture.handler(escape).is_err());
        fixture.owner.enable_cancel(Some(1))?;
        let handler = fixture.handler(escape)?;
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        fixture.owner.enable_cancel(Some(1))?;
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.cancels.load(Ordering::Acquire), 1);
        handler(ShortcutState::Released, NativeOrigin::now(false));
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.cancels.load(Ordering::Acquire), 2);
        fixture.owner.enable_cancel(None)?;
        handler(ShortcutState::Released, NativeOrigin::now(false));
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.cancels.load(Ordering::Acquire), 2);
        assert!(fixture.handler(escape).is_err());
        Ok(())
    }

    #[test]
    fn release_while_escape_is_unregistered_rearms_first_press_in_next_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        let escape = Shortcut::new(None, Code::Escape);
        fixture.owner.enable_cancel(Some(11))?;
        fixture.handler(escape)?(ShortcutState::Pressed, NativeOrigin::now(false));
        fixture.owner.enable_cancel(None)?;
        // The OS does not deliver a shortcut release while it is unregistered.
        fixture.state.lock().map_err(|_| "lock")?.escape_pressed = false;
        fixture.owner.enable_cancel(Some(12))?;
        fixture.handler(escape)?(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(
            *fixture.cancelled_sessions.lock().map_err(|_| "lock")?,
            vec![11, 12]
        );
        Ok(())
    }

    #[test]
    fn physical_escape_held_before_registration_stays_suppressed_across_idle()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        let escape = Shortcut::new(None, Code::Escape);
        fixture.state.lock().map_err(|_| "lock")?.escape_pressed = true;
        fixture.owner.enable_cancel(Some(11))?;
        let handler = fixture.handler(escape)?;
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert!(
            fixture
                .cancelled_sessions
                .lock()
                .map_err(|_| "lock")?
                .is_empty()
        );
        handler(ShortcutState::Released, NativeOrigin::now(false));
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        fixture.owner.enable_cancel(None)?;
        fixture.owner.enable_cancel(Some(12))?;
        let handler = fixture.handler(escape)?;
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(
            *fixture.cancelled_sessions.lock().map_err(|_| "lock")?,
            vec![11]
        );
        handler(ShortcutState::Released, NativeOrigin::now(false));
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(
            *fixture.cancelled_sessions.lock().map_err(|_| "lock")?,
            vec![11, 12]
        );
        Ok(())
    }

    #[test]
    fn held_escape_does_not_cancel_new_session_until_released()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        let escape = Shortcut::new(None, Code::Escape);
        fixture.owner.enable_cancel(Some(11))?;
        fixture.handler(escape)?(ShortcutState::Pressed, NativeOrigin::now(false));
        fixture.owner.enable_cancel(Some(12))?;
        let new_handler = fixture.handler(escape)?;
        new_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(
            *fixture.cancelled_sessions.lock().map_err(|_| "lock")?,
            vec![11]
        );
        new_handler(ShortcutState::Released, NativeOrigin::now(false));
        new_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(
            *fixture.cancelled_sessions.lock().map_err(|_| "lock")?,
            vec![11, 12]
        );
        Ok(())
    }

    #[test]
    fn changing_active_session_revokes_old_escape_and_captures_new_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        let escape = Shortcut::new(None, Code::Escape);
        fixture.owner.enable_cancel(Some(11))?;
        let old_handler = fixture.handler(escape)?;
        fixture.owner.enable_cancel(Some(12))?;
        old_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        let new_handler = fixture.handler(escape)?;
        new_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(
            *fixture.cancelled_sessions.lock().map_err(|_| "lock")?,
            vec![12]
        );
        Ok(())
    }

    #[test]
    fn successful_rebind_revokes_queued_old_handler() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        let old = Shortcut::new(Some(Modifiers::ALT), Code::KeyD);
        assert_eq!(
            fixture.owner.current_binding(),
            Some(crate::settings::Settings::default().hotkey)
        );
        let old_handler = fixture.handler(old)?;
        fixture.owner.reconfigure(Hotkey {
            key: PhysicalKey::G,
            ..crate::settings::Settings::default().hotkey
        })?;
        assert_eq!(
            fixture.owner.current_binding(),
            Some(Hotkey {
                key: PhysicalKey::G,
                ..crate::settings::Settings::default().hotkey
            })
        );
        assert!(fixture.handler(old).is_err());
        old_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        let new_handler = fixture.handler(Shortcut::new(Some(Modifiers::ALT), Code::KeyG))?;
        new_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.toggles.load(Ordering::Acquire), 1);
        Ok(())
    }

    #[test]
    fn failed_rebind_preserves_old_key_and_cleans_only_owned_keys()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        let old = Shortcut::new(Some(Modifiers::ALT), Code::KeyD);
        let replacement = Hotkey {
            key: PhysicalKey::G,
            ..crate::settings::Settings::default().hotkey
        };
        let new = Shortcut::new(Some(Modifiers::ALT), Code::KeyG);
        let old_handler = fixture.handler(old)?;
        fixture.state.lock().map_err(|_| "lock")?.fail_register = true;
        assert!(fixture.owner.reconfigure(replacement.clone()).is_err());
        assert_eq!(
            fixture.owner.current_binding(),
            Some(crate::settings::Settings::default().hotkey)
        );
        old_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.toggles.load(Ordering::Acquire), 1);
        old_handler(ShortcutState::Released, NativeOrigin::now(false));
        {
            let mut state = fixture.state.lock().map_err(|_| "lock")?;
            state.fail_register = false;
            state.fail_remove = vec![old.id(), new.id()];
        }
        assert!(fixture.owner.reconfigure(replacement.clone()).is_err());
        assert_eq!(
            fixture.owner.current_binding(),
            Some(crate::settings::Settings::default().hotkey)
        );
        let abandoned_handler = fixture.handler(new)?;
        abandoned_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        old_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.toggles.load(Ordering::Acquire), 2);
        // A failed rollback owns one disabled registration; retry its removal
        // before another replacement, so failures cannot grow an unbounded list.
        assert!(fixture.owner.reconfigure(replacement).is_err());
        assert_eq!(
            fixture.owner.current_binding(),
            Some(crate::settings::Settings::default().hotkey)
        );
        fixture
            .state
            .lock()
            .map_err(|_| "lock")?
            .fail_remove
            .clear();
        fixture.owner.shutdown()?;
        assert_eq!(fixture.owner.current_binding(), None);
        assert!(
            fixture
                .state
                .lock()
                .map_err(|_| "lock")?
                .handlers
                .is_empty()
        );
        old_handler(ShortcutState::Released, NativeOrigin::now(false));
        old_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.toggles.load(Ordering::Acquire), 2);
        assert!(matches!(
            fixture.owner.enable_cancel(Some(1)),
            Err(HotkeyError::Closed)
        ));
        Ok(())
    }
    #[test]
    fn failed_escape_removal_disables_actions_and_shutdown_retries_both_keys()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        fixture.owner.enable_cancel(Some(1))?;
        let escape = Shortcut::new(None, Code::Escape);
        let toggle = Shortcut::new(Some(Modifiers::ALT), Code::KeyD);
        let escape_handler = fixture.handler(escape)?;
        fixture
            .state
            .lock()
            .map_err(|_| "lock")?
            .fail_remove
            .push(escape.id());
        assert!(fixture.owner.enable_cancel(None).is_err());
        assert!(fixture.owner.enable_cancel(Some(1)).is_err());
        escape_handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.cancels.load(Ordering::Acquire), 0);
        assert!(fixture.owner.shutdown().is_err());
        assert!(fixture.handler(toggle).is_err());
        fixture
            .state
            .lock()
            .map_err(|_| "lock")?
            .fail_remove
            .clear();
        fixture.owner.shutdown()?;
        assert_eq!(fixture.owner.current_binding(), None);
        assert!(
            fixture
                .state
                .lock()
                .map_err(|_| "lock")?
                .handlers
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn failed_shutdown_reports_no_binding_while_retaining_native_cleanup_ownership()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = Fixture::new()?;
        let toggle = Shortcut::new(Some(Modifiers::ALT), Code::KeyD);
        let handler = fixture.handler(toggle)?;
        fixture
            .state
            .lock()
            .map_err(|_| "lock")?
            .fail_remove
            .push(toggle.id());
        assert!(fixture.owner.shutdown().is_err());
        assert!(fixture.handler(toggle).is_ok());
        assert_eq!(fixture.owner.current_binding(), None);
        handler(ShortcutState::Pressed, NativeOrigin::now(false));
        assert_eq!(fixture.toggles.load(Ordering::Acquire), 0);
        fixture
            .state
            .lock()
            .map_err(|_| "lock")?
            .fail_remove
            .clear();
        fixture.owner.shutdown()?;
        assert_eq!(fixture.owner.current_binding(), None);
        assert!(fixture.handler(toggle).is_err());
        Ok(())
    }

    #[test]
    fn each_binding_allows_only_one_pending_main_thread_callback() {
        let gate = Arc::new(Gate::default());
        let first = gate.reserve();
        assert!(first.is_some());
        assert!(gate.reserve().is_none());
        drop(first);
        assert!(gate.reserve().is_some());
    }

    #[test]
    fn repeated_press_has_one_action_until_release() {
        let gate = Gate::default();
        gate.enabled.store(true, Ordering::Release);
        assert!(gate.accept(ShortcutState::Pressed));
        assert!(!gate.accept(ShortcutState::Pressed));
        assert!(!gate.accept(ShortcutState::Released));
        assert!(gate.accept(ShortcutState::Pressed));
        gate.enabled.store(false, Ordering::Release);
        assert!(!gate.accept(ShortcutState::Pressed));
    }
}
