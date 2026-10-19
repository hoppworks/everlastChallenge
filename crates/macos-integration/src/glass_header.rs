//! Full-width public AppKit glass controls, independent of renderer transparency.
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, rc::Retained,
    runtime::AnyObject, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSButton, NSEventModifierFlags, NSGlassEffectView,
    NSGlassEffectViewStyle, NSLayoutAttribute, NSTextField, NSTitlebarAccessoryViewController,
    NSView, NSWindow,
};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    marker::PhantomData,
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

const HEADER_HEIGHT: f64 = 64.0;
const MAX_STATUS_BYTES: usize = 512;
thread_local! { static OWNERS: RefCell<BTreeSet<usize>> = const { RefCell::new(BTreeSet::new()) }; }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    Start,
    Stop(NonZeroU64),
    Cancel(NonZeroU64),
    Settings,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderPrimary {
    Start,
    Stop(NonZeroU64),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderState {
    pub status: String,
    pub primary: Option<HeaderPrimary>,
    pub cancel: Option<NonZeroU64>,
    pub settings_enabled: bool,
}
impl Default for HeaderState {
    fn default() -> Self {
        Self {
            status: "Status wird geladen".into(),
            primary: None,
            cancel: None,
            settings_enabled: false,
        }
    }
}
impl HeaderState {
    fn validate(&self) -> Result<(), HeaderError> {
        if self.status.trim().is_empty()
            || self.status.len() > MAX_STATUS_BYTES
            || self.status.contains('\0')
        {
            return Err(HeaderError::State);
        }
        match (self.primary, self.cancel) {
            (Some(HeaderPrimary::Start), Some(_)) => Err(HeaderError::State),
            (Some(HeaderPrimary::Stop(id)), Some(cancel)) if id != cancel => {
                Err(HeaderError::State)
            }
            _ => Ok(()),
        }
    }
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HeaderError {
    #[error("Glass headers require the native main thread")]
    MainThread,
    #[error("The AppKit window handle is unavailable")]
    Handle,
    #[error("The view is detached from its window")]
    Detached,
    #[error("This window already has an owned glass header")]
    Busy,
    #[error("The owned header hierarchy changed")]
    Changed,
    #[error("The header state is invalid or too large")]
    State,
    #[error("The header is no longer active")]
    Inactive,
}

#[derive(Clone, Copy)]
enum ActionKind {
    Primary,
    Cancel,
    Settings,
}
struct Core {
    state: RefCell<HeaderState>,
    active: Cell<bool>,
    callback: Box<dyn Fn(HeaderAction) + Send + Sync>,
}
impl Core {
    fn dispatch(&self, kind: ActionKind) {
        if !self.active.get() {
            return;
        }
        let action = {
            let state = self.state.borrow();
            match kind {
                ActionKind::Primary => state.primary.map(|primary| match primary {
                    HeaderPrimary::Start => HeaderAction::Start,
                    HeaderPrimary::Stop(id) => HeaderAction::Stop(id),
                }),
                ActionKind::Cancel => state.cancel.map(HeaderAction::Cancel),
                ActionKind::Settings => state.settings_enabled.then_some(HeaderAction::Settings),
            }
        };
        if let Some(action) = action
            && catch_unwind(AssertUnwindSafe(|| (self.callback)(action))).is_err()
        {
            self.active.set(false);
            eprintln!("Native header callback failed; actions disabled");
        }
    }
}
struct ActionIvars {
    core: Rc<Core>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ActionIvars]
    struct HeaderActionTarget;
    impl HeaderActionTarget {
        #[unsafe(method(primary:))]
        fn primary(&self, _sender: Option<&AnyObject>) { let _keep_alive = self.retain(); self.ivars().core.dispatch(ActionKind::Primary); }
        #[unsafe(method(cancel:))]
        fn cancel(&self, _sender: Option<&AnyObject>) { let _keep_alive = self.retain(); self.ivars().core.dispatch(ActionKind::Cancel); }
        #[unsafe(method(settings:))]
        fn settings(&self, _sender: Option<&AnyObject>) { let _keep_alive = self.retain(); self.ivars().core.dispatch(ActionKind::Settings); }
    }
);
impl HeaderActionTarget {
    fn new(main: MainThreadMarker, core: Rc<Core>) -> Retained<Self> {
        let value = Self::alloc(main).set_ivars(ActionIvars { core });
        // SAFETY: NSObject's designated init initializes our freshly allocated
        // subclass; ivars already own all Rust state for the object's lifetime.
        unsafe { msg_send![super(value), init] }
    }
}

struct Views {
    status: Retained<NSTextField>,
    settings: Retained<NSButton>,
    primary: Retained<NSButton>,
    cancel: Retained<NSButton>,
}
define_class!(
    #[unsafe(super(NSView))]
    #[ivars = Views]
    struct HeaderContent;
    impl HeaderContent {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool { true }
        #[unsafe(method(layout))]
        fn native_layout(&self) {
            // SAFETY: Call the superclass layout implementation on this NSView.
            unsafe { let _: () = msg_send![super(self), layout]; }
            self.arrange();
        }
        #[unsafe(method(resizeSubviewsWithOldSize:))]
        fn resized(&self, size: NSSize) {
            // SAFETY: Preserve NSView's normal autoresizing before our fixed,
            // bounded single-row layout; dimensions come from AppKit itself.
            unsafe { let _: () = msg_send![super(self), resizeSubviewsWithOldSize: size]; }
            self.arrange();
        }
    }
);
impl HeaderContent {
    fn new(main: MainThreadMarker, views: Views, width: f64) -> Retained<Self> {
        let value = Self::alloc(main).set_ivars(views);
        // SAFETY: Designated NSView initializer on a newly allocated subclass.
        let view: Retained<Self> = unsafe {
            msg_send![super(value), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, HEADER_HEIGHT))]
        };
        for child in [
            &*view.ivars().status as &NSView,
            &*view.ivars().settings,
            &*view.ivars().primary,
            &*view.ivars().cancel,
        ] {
            view.addSubview(child);
        }
        view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        view.arrange();
        view
    }
    fn arrange(&self) {
        let width = self.bounds().size.width.max(0.0);
        let views = self.ivars();
        let mut edge = width - 14.0;
        // Settings is always reachable. Only present session actions reserve
        // width, so the status recovers space as Start/Stop/Cancel disappear.
        for (button, button_width) in [
            (&views.settings, 116.0),
            (&views.cancel, 100.0),
            (&views.primary, 104.0),
        ] {
            if !button.isHidden() {
                edge -= button_width;
                button.setFrame(NSRect::new(
                    NSPoint::new(edge.max(14.0), 16.0),
                    NSSize::new(button_width, 32.0),
                ));
                edge -= 8.0;
            }
        }
        views.status.setFrame(NSRect::new(
            NSPoint::new(14.0, 12.0),
            NSSize::new((edge - 18.0).max(0.0), 40.0),
        ));
    }
    fn disable(&self) {
        for button in [
            &self.ivars().settings,
            &self.ivars().primary,
            &self.ivars().cancel,
        ] {
            button.setEnabled(false);
            // SAFETY: Clear the owned target/action before their owner can die;
            // this also makes retained old controls inert after detachment.
            unsafe {
                button.setTarget(None);
                button.setAction(None);
            }
        }
    }
}

/// Own on main; explicit detach is required before process-exit cleanup.
/// Native objects keep this guard !Send and !Sync even if W is Send + Sync.
///
/// ```compile_fail
/// use macos_integration::glass_header::GlassHeader;
/// use raw_window_handle::HasWindowHandle;
/// fn require_send<T: Send>() {}
/// fn check<W: HasWindowHandle + Send + Sync + 'static>() {
///     require_send::<GlassHeader<W>>();
/// }
/// ```
/// ```compile_fail
/// use macos_integration::glass_header::GlassHeader;
/// use raw_window_handle::HasWindowHandle;
/// fn require_sync<T: Sync>() {}
/// fn check<W: HasWindowHandle + Send + Sync + 'static>() {
///     require_sync::<GlassHeader<W>>();
/// }
/// ```
pub struct GlassHeader<W: HasWindowHandle + 'static> {
    window: Retained<NSWindow>,
    accessory: Retained<NSTitlebarAccessoryViewController>,
    glass: Retained<NSGlassEffectView>,
    content: Retained<HeaderContent>,
    core: Rc<Core>,
    _target: Retained<HeaderActionTarget>,
    registered: bool,
    attached: bool,
    _main: PhantomData<Rc<()>>,
    _owner: W,
}
impl<W: HasWindowHandle + 'static> GlassHeader<W> {
    pub fn attach(
        owner: W,
        callback: impl Fn(HeaderAction) + Send + Sync + 'static,
    ) -> Result<Self, HeaderError> {
        let main = MainThreadMarker::new().ok_or(HeaderError::MainThread)?;
        let original = {
            let handle = owner.window_handle().map_err(|_| HeaderError::Handle)?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return Err(HeaderError::Handle);
            };
            // SAFETY: A borrowed AppKit WindowHandle guarantees this NSView is
            // valid. Retain before that borrow ends; owner stays in this guard.
            unsafe { Retained::<NSView>::retain(handle.ns_view.as_ptr().cast()) }
                .ok_or(HeaderError::Handle)?
        };
        let window = original.window().ok_or(HeaderError::Detached)?;
        let key = Retained::as_ptr(&window) as usize;
        if OWNERS.with(|owners| owners.borrow().contains(&key)) {
            return Err(HeaderError::Busy);
        }
        let core = Rc::new(Core {
            state: RefCell::new(HeaderState::default()),
            active: Cell::new(false),
            callback: Box::new(callback),
        });
        let target = HeaderActionTarget::new(main, core.clone());
        let button = |title: &str, action| {
            // SAFETY: target is our retained NSObject subclass and each selector
            // below is a declared one-argument action on that exact class.
            unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(title),
                    Some(&target),
                    Some(action),
                    main,
                )
            }
        };
        let status =
            NSTextField::wrappingLabelWithString(&NSString::from_str("Status wird geladen"), main);
        status.setSelectable(false);
        status.setMaximumNumberOfLines(2);
        status.setAccessibilityLabel(Some(&NSString::from_str("Diktierstatus")));
        let primary = button("Aufnehmen", sel!(primary:));
        primary.setKeyEquivalent(&NSString::from_str("r"));
        primary.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        let cancel = button("Abbrechen", sel!(cancel:));
        cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        cancel.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
        cancel.setToolTip(Some(&NSString::from_str("Aufnahme abbrechen (Esc)")));
        let settings = button("Einstellungen", sel!(settings:));
        settings.setKeyEquivalent(&NSString::from_str(","));
        settings.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        let content = HeaderContent::new(
            main,
            Views {
                status,
                settings,
                primary,
                cancel,
            },
            original.bounds().size.width,
        );
        let glass = NSGlassEffectView::new(main);
        glass.setFrame(content.frame());
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setContentView(Some(&content));
        glass.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        let accessory = NSTitlebarAccessoryViewController::new(main);
        accessory.setLayoutAttribute(NSLayoutAttribute::Bottom);
        accessory.setAutomaticallyAdjustsSize(false);
        accessory.setView(&glass);
        if !OWNERS.with(|owners| owners.borrow_mut().insert(key)) {
            return Err(HeaderError::Busy);
        }
        let header = Self {
            window,
            accessory,
            glass,
            content,
            core,
            _target: target,
            registered: true,
            attached: true,
            _main: PhantomData,
            _owner: owner,
        };
        header
            .window
            .addTitlebarAccessoryViewController(&header.accessory);
        header.core.active.set(true);
        header.apply_controls(&HeaderState::default());
        if !header.owns_hierarchy() {
            return Err(HeaderError::Changed);
        }
        Ok(header)
    }
    fn owns_hierarchy(&self) -> bool {
        self.window
            .titlebarAccessoryViewControllers()
            .iter()
            .any(|item| std::ptr::eq(&*item, &*self.accessory))
            && std::ptr::eq(&*self.accessory.view(), &*self.glass as &NSView)
            && self
                .glass
                .contentView()
                .is_some_and(|view| std::ptr::eq(&*view, &*self.content as &NSView))
            && self.content.isDescendantOf(&self.glass)
    }
    fn apply_controls(&self, state: &HeaderState) {
        let views = self.content.ivars();
        views
            .status
            .setStringValue(&NSString::from_str(&state.status));
        views
            .status
            .setToolTip(Some(&NSString::from_str(&state.status)));
        views.primary.setTitle(&NSString::from_str(
            if matches!(state.primary, Some(HeaderPrimary::Stop(_))) {
                "Stoppen"
            } else {
                "Aufnehmen"
            },
        ));
        views.primary.setToolTip(Some(&NSString::from_str(
            if matches!(state.primary, Some(HeaderPrimary::Stop(_))) {
                "Aufnahme stoppen (⌘R)"
            } else {
                "Aufnahme starten (⌘R)"
            },
        )));
        views.primary.setEnabled(state.primary.is_some());
        views.primary.setHidden(state.primary.is_none());
        views.cancel.setEnabled(state.cancel.is_some());
        views.cancel.setHidden(state.cancel.is_none());
        views.settings.setEnabled(state.settings_enabled);
        self.content.arrange();
    }
    pub fn update(&mut self, state: HeaderState) -> Result<(), HeaderError> {
        state.validate()?;
        if !self.attached || !self.core.active.get() {
            return Err(HeaderError::Inactive);
        }
        if !self.owns_hierarchy() {
            self.core.active.set(false);
            self.content.disable();
            return Err(HeaderError::Changed);
        }
        self.apply_controls(&state);
        self.core.state.replace(state);
        Ok(())
    }
    pub fn detach(&mut self) -> Result<(), HeaderError> {
        self.core.active.set(false);
        self.content.disable();
        if !self.attached {
            return Ok(());
        }
        if !self.owns_hierarchy() {
            return Err(HeaderError::Changed);
        }
        self.accessory.removeFromParentViewController();
        self.attached = false;
        self.release();
        Ok(())
    }
    fn release(&mut self) {
        if self.registered {
            let removed = OWNERS.with(|owners| {
                owners
                    .borrow_mut()
                    .remove(&(Retained::as_ptr(&self.window) as usize))
            });
            debug_assert!(removed);
            self.registered = false;
        }
    }
}
impl<W: HasWindowHandle + 'static> Drop for GlassHeader<W> {
    fn drop(&mut self) {
        if self.detach().is_err() {
            eprintln!("Native header hierarchy changed; foreign accessories preserved");
        }
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Unavailable(Arc<AtomicUsize>);
    impl HasWindowHandle for Unavailable {
        fn window_handle(
            &self,
        ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(raw_window_handle::HandleError::Unavailable)
        }
    }
    #[test]
    fn rejects_workers_before_touching_native_handles() -> Result<(), Box<dyn std::error::Error>> {
        let calls = Arc::new(AtomicUsize::new(0));
        let owner = Unavailable(calls.clone());
        let error = std::thread::spawn(|| GlassHeader::attach(owner, |_| {}).err())
            .join()
            .map_err(|_| "worker panicked")?;
        assert_eq!(error, Some(HeaderError::MainThread));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        Ok(())
    }
}
