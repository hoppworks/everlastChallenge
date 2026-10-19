//! Native main workspace. The host supplies state; actions never invent session truth.
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, rc::Retained,
    runtime::AnyObject, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertSecondButtonReturn, NSAlertStyle, NSAutoresizingMaskOptions,
    NSBorderType, NSButton, NSColor, NSFont, NSLayoutConstraint, NSLayoutXAxisAnchor,
    NSLayoutYAxisAnchor, NSLevelIndicator, NSProgressIndicator, NSRectFill, NSResponder,
    NSScrollView, NSTextField, NSTextView, NSView, NSWindow,
};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceAction {
    Copy,
    Clear,
    Download,
    CancelSetup,
    Settings,
    ResetSettingsConfirmed,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingReadout {
    pub elapsed_seconds: u32,
    pub level_permille: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadReadout {
    pub received: u64,
    pub total: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceState {
    pub status: String,
    pub detail: String,
    pub setup_info: Option<String>,
    pub permission_info: Option<String>,
    pub error: Option<String>,
    pub transcript: String,
    pub hotkey: String,
    pub recording: Option<RecordingReadout>,
    pub download: Option<DownloadReadout>,
    pub recovery: Option<String>,
    pub can_copy: bool,
    pub can_clear: bool,
    pub can_download: bool,
    pub can_cancel_setup: bool,
    pub can_settings: bool,
}
impl WorkspaceState {
    fn validate(&self) -> Result<(), WorkspaceError> {
        let labels = [&self.status, &self.detail, &self.hotkey];
        if labels
            .into_iter()
            .chain(self.error.iter())
            .chain(self.recovery.iter())
            .chain(self.setup_info.iter())
            .chain(self.permission_info.iter())
            .any(|value| value.len() > 2048 || value.contains('\0'))
            || self.transcript.len() > 256 * 1024
            || self.transcript.contains('\0')
            || self
                .recording
                .as_ref()
                .is_some_and(|value| value.level_permille > 1000)
            || self.download.as_ref().is_some_and(|value| {
                value
                    .total
                    .is_some_and(|total| total == 0 || value.received > total)
            })
            || (self.recording.is_some() && self.download.is_some())
        {
            return Err(WorkspaceError::State);
        }
        Ok(())
    }
    fn allows(&self, action: WorkspaceAction) -> bool {
        if self.recovery.is_some() {
            return action == WorkspaceAction::ResetSettingsConfirmed;
        }
        match action {
            WorkspaceAction::Copy => self.can_copy && !self.transcript.is_empty(),
            WorkspaceAction::Clear => self.can_clear && !self.transcript.is_empty(),
            WorkspaceAction::Download => self.can_download,
            WorkspaceAction::CancelSetup => self.can_cancel_setup,
            WorkspaceAction::Settings => self.can_settings,
            WorkspaceAction::ResetSettingsConfirmed => false,
        }
    }
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkspaceError {
    #[error("Native workspace requires the main thread")]
    MainThread,
    #[error("The AppKit content handle is unavailable")]
    Handle,
    #[error("The content view is detached")]
    Detached,
    #[error("The window already has a native workspace")]
    Busy,
    #[error("The owned workspace hierarchy changed")]
    Changed,
    #[error("The workspace state is invalid or too large")]
    State,
    #[error("The workspace is inactive")]
    Inactive,
}
thread_local! { static OWNERS: RefCell<BTreeSet<usize>> = const { RefCell::new(BTreeSet::new()) }; }
struct Core {
    state: RefCell<WorkspaceState>,
    active: Cell<bool>,
    callback: Box<dyn Fn(WorkspaceAction) + Send + Sync>,
}
impl Core {
    fn dispatch(&self, action: WorkspaceAction) {
        if !self.active.get() || !self.state.borrow().allows(action) {
            return;
        }
        if catch_unwind(AssertUnwindSafe(|| (self.callback)(action))).is_err() {
            self.active.set(false);
            eprintln!("Native workspace callback failed; actions disabled");
        }
    }
    fn reset_with_confirmation(&self, confirm: impl FnOnce() -> bool) {
        if !self.active.get() {
            return;
        }
        let expected = self.state.borrow().recovery.clone();
        if expected.is_some() && confirm() && self.state.borrow().recovery == expected {
            self.dispatch(WorkspaceAction::ResetSettingsConfirmed);
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
    struct WorkspaceTarget;
    impl WorkspaceTarget {
        #[unsafe(method(copyText:))]
        fn copy_text(&self, _: Option<&AnyObject>) { let _owner = self.retain(); self.ivars().core.dispatch(WorkspaceAction::Copy); }
        #[unsafe(method(clearText:))]
        fn clear_text(&self, _: Option<&AnyObject>) { let _owner = self.retain(); self.ivars().core.dispatch(WorkspaceAction::Clear); }
        #[unsafe(method(download:))]
        fn download(&self, _: Option<&AnyObject>) { let _owner = self.retain(); self.ivars().core.dispatch(WorkspaceAction::Download); }
        #[unsafe(method(cancelSetup:))]
        fn cancel_setup(&self, _: Option<&AnyObject>) { let _owner = self.retain(); self.ivars().core.dispatch(WorkspaceAction::CancelSetup); }
        #[unsafe(method(settings:))]
        fn settings(&self, _: Option<&AnyObject>) { let _owner = self.retain(); self.ivars().core.dispatch(WorkspaceAction::Settings); }
        #[unsafe(method(resetSettings:))]
        fn reset_settings(&self, _: Option<&AnyObject>) {
            let _owner = self.retain();
            self.ivars().core.reset_with_confirmation(|| {
                let Some(main) = MainThreadMarker::new() else { return false; };
                let alert = NSAlert::new(main);
                alert.setAlertStyle(NSAlertStyle::Warning);
                alert.setMessageText(&NSString::from_str("Einstellungen zurücksetzen?"));
                alert.setInformativeText(&NSString::from_str("Mikrofon, Sprache, Tastenkürzel und Modellauswahl werden auf Standardwerte gesetzt. Bereits geladene Sprachmodelle bleiben erhalten."));
                // Cancellation is deliberately the default/Return action.
                let _cancel = alert.addButtonWithTitle(&NSString::from_str("Abbrechen"));
                let _reset = alert.addButtonWithTitle(&NSString::from_str("Zurücksetzen"));
                alert.runModal() == NSAlertSecondButtonReturn
            });
        }
    }
);
impl WorkspaceTarget {
    fn new(main: MainThreadMarker, core: Rc<Core>) -> Retained<Self> {
        let value = Self::alloc(main).set_ivars(ActionIvars { core });
        // SAFETY: designated NSObject initialization, after our owned Rust ivars.
        unsafe { msg_send![super(value), init] }
    }
}
struct Views {
    setup_info: Retained<NSTextField>,
    permission_info: Retained<NSTextField>,
    status: Retained<NSTextField>,
    detail: Retained<NSTextField>,
    error: Retained<NSTextField>,
    instruction: Retained<NSTextField>,
    readout: Retained<NSTextField>,
    level: Retained<NSLevelIndicator>,
    progress: Retained<NSProgressIndicator>,
    bytes: Retained<NSTextField>,
    download: Retained<NSButton>,
    cancel_setup: Retained<NSButton>,
    settings: Retained<NSButton>,
    reset: Retained<NSButton>,
    title: Retained<NSTextField>,
    copy: Retained<NSButton>,
    clear: Retained<NSButton>,
    scroll: Retained<NSScrollView>,
    text: Retained<NSTextView>,
}
define_class!(
    #[unsafe(super(NSView))]
    #[ivars = Views]
    struct WorkspaceBody;
    impl WorkspaceBody {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool { true }
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool { true }
        #[unsafe(method(drawRect:))]
        fn draw_background(&self, _rect: NSRect) {
            NSColor::windowBackgroundColor().setFill();
            // Dirty rectangles can extend into full-size window chrome. Paint
            // only this view; clipping also contains descendants on macOS 14+.
            NSRectFill(self.bounds());
        }
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn changed_appearance(&self) {
            // SAFETY: retain NSView's inherited public appearance handling.
            unsafe { let _: () = msg_send![super(self), viewDidChangeEffectiveAppearance]; }
            self.setNeedsDisplay(true);
        }
        #[unsafe(method(layout))]
        fn native_layout(&self) {
            // SAFETY: preserve NSView's public layout on this initialized subclass.
            unsafe { let _: () = msg_send![super(self), layout]; }
            self.arrange();
        }
        #[unsafe(method(resizeSubviewsWithOldSize:))]
        fn resized(&self, size: NSSize) {
            // SAFETY: forward AppKit's native size to the inherited NSView method.
            unsafe { let _: () = msg_send![super(self), resizeSubviewsWithOldSize: size]; }
            self.arrange();
        }
    }
);
impl WorkspaceBody {
    fn new(main: MainThreadMarker, views: Views, frame: NSRect) -> Retained<Self> {
        let value = Self::alloc(main).set_ivars(views);
        // SAFETY: NSView's designated initializer on our newly allocated subclass.
        let body: Retained<Self> = unsafe { msg_send![super(value), initWithFrame: frame] };
        body.setClipsToBounds(true);
        let v = body.ivars();
        for child in [
            &*v.status as &NSView,
            &*v.detail,
            &*v.error,
            &*v.instruction,
            &*v.setup_info,
            &*v.permission_info,
            &*v.readout,
            &*v.level,
            &*v.progress,
            &*v.bytes,
            &*v.download,
            &*v.cancel_setup,
            &*v.settings,
            &*v.reset,
            &*v.title,
            &*v.copy,
            &*v.clear,
            &*v.scroll,
        ] {
            body.addSubview(child);
        }
        body.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        body.arrange();
        body
    }
    fn arrange(&self) {
        let v = self.ivars();
        let width = (self.bounds().size.width - 40.0).max(0.0);
        let mut y = 18.0;
        for (label, height) in [
            (&v.status, 28.0),
            (&v.detail, 36.0),
            (&v.error, 40.0),
            (&v.instruction, 44.0),
            (&v.setup_info, 60.0),
            (&v.permission_info, 40.0),
        ] {
            if !label.isHidden() {
                label.setFrame(NSRect::new(
                    NSPoint::new(20.0, y),
                    NSSize::new(width, height),
                ));
                y += height + 8.0;
            }
        }
        if !v.readout.isHidden() {
            v.readout
                .setFrame(NSRect::new(NSPoint::new(20.0, y), NSSize::new(120.0, 24.0)));
            v.level.setFrame(NSRect::new(
                NSPoint::new(154.0, y + 4.0),
                NSSize::new((width - 134.0).max(0.0), 16.0),
            ));
            y += 32.0;
        }
        if !v.progress.isHidden() {
            v.bytes
                .setFrame(NSRect::new(NSPoint::new(20.0, y), NSSize::new(width, 20.0)));
            v.progress.setFrame(NSRect::new(
                NSPoint::new(20.0, y + 22.0),
                NSSize::new(width, 12.0),
            ));
            y += 42.0;
        }
        let mut x = 20.0;
        for (button, button_width) in [
            (&v.download, 130.0),
            (&v.cancel_setup, 184.0),
            (&v.settings, 186.0),
            (&v.reset, 220.0),
        ] {
            if !button.isHidden() {
                if x > 20.0 && x + button_width > width + 20.0 {
                    x = 20.0;
                    y += 40.0;
                }
                button.setFrame(NSRect::new(
                    NSPoint::new(x, y),
                    NSSize::new(button_width, 32.0),
                ));
                x += button_width + 10.0;
            }
        }
        if x > 20.0 {
            y += 44.0;
        }
        v.title.setFrame(NSRect::new(
            NSPoint::new(20.0, y + 6.0),
            NSSize::new((width - 194.0).max(0.0), 22.0),
        ));
        v.copy.setFrame(NSRect::new(
            NSPoint::new(width - 164.0, y),
            NSSize::new(88.0, 30.0),
        ));
        v.clear.setFrame(NSRect::new(
            NSPoint::new(width - 66.0, y),
            NSSize::new(86.0, 30.0),
        ));
        y += 38.0;
        let height = (self.bounds().size.height - y - 18.0).max(48.0);
        v.scroll.setFrame(NSRect::new(
            NSPoint::new(20.0, y),
            NSSize::new(width, height),
        ));
        let visible = v.scroll.contentSize();
        v.text.setMinSize(NSSize::new(0.0, visible.height));
        v.text.setFrameSize(NSSize::new(
            visible.width,
            v.text.frame().size.height.max(visible.height),
        ));
    }
    fn disable(&self) {
        let v = self.ivars();
        for button in [
            &v.copy,
            &v.clear,
            &v.download,
            &v.cancel_setup,
            &v.settings,
            &v.reset,
        ] {
            button.setEnabled(false);
            // SAFETY: these owned controls only reference our retained target;
            // clearing it makes any externally retained old control inert.
            unsafe {
                button.setTarget(None);
                button.setAction(None);
            }
        }
        // SAFETY: nil sender is allowed for this public animation stop action.
        unsafe {
            v.progress.stopAnimation(None);
        }
    }
}
/// Main-thread owned native sibling of the retained webview. Detach before the
/// Tauri window is destroyed. Updates never steal focus or reset text selection
/// when the transcript did not change. No target app, audio or clipboard is read.
pub struct NativeWorkspace<O: HasWindowHandle + 'static> {
    window: Retained<NSWindow>,
    original: Retained<NSView>,
    parent: Retained<NSView>,
    _layout_guide: Retained<AnyObject>,
    constraints: [Retained<NSLayoutConstraint>; 4],
    original_hidden: bool,
    responder: Option<Retained<NSResponder>>,
    body: Retained<WorkspaceBody>,
    core: Rc<Core>,
    _target: Retained<WorkspaceTarget>,
    attached: bool,
    registered: bool,
    _main: PhantomData<Rc<()>>,
    _owner: O,
}
impl<O: HasWindowHandle + 'static> NativeWorkspace<O> {
    pub fn attach(
        owner: O,
        callback: impl Fn(WorkspaceAction) + Send + Sync + 'static,
    ) -> Result<Self, WorkspaceError> {
        let main = MainThreadMarker::new().ok_or(WorkspaceError::MainThread)?;
        let original = {
            let handle = owner.window_handle().map_err(|_| WorkspaceError::Handle)?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return Err(WorkspaceError::Handle);
            };
            // SAFETY: borrowed WindowHandle guarantees the native NSView remains
            // valid. Retain before the borrow ends; owner remains in this guard.
            unsafe { Retained::<NSView>::retain(handle.ns_view.as_ptr().cast()) }
                .ok_or(WorkspaceError::Handle)?
        };
        let window = original.window().ok_or(WorkspaceError::Detached)?;
        let parent = parent_of(&original).ok_or(WorkspaceError::Detached)?;
        let guide = window
            .contentLayoutGuide()
            .ok_or(WorkspaceError::Detached)?;
        let key = Retained::as_ptr(&window) as usize;
        if OWNERS.with(|owners| owners.borrow().contains(&key)) {
            return Err(WorkspaceError::Busy);
        }
        let core = Rc::new(Core {
            state: RefCell::new(WorkspaceState::default()),
            active: Cell::new(false),
            callback: Box::new(callback),
        });
        let target = WorkspaceTarget::new(main, core.clone());
        let label = |value: &str| {
            let field = NSTextField::wrappingLabelWithString(&NSString::from_str(value), main);
            field.setSelectable(false);
            field.setMaximumNumberOfLines(2);
            field.setFont(Some(&NSFont::systemFontOfSize(13.0)));
            field
        };
        let button = |title: &str, selector| {
            // SAFETY: each selector is declared on our retained target above.
            unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(title),
                    Some(&target),
                    Some(selector),
                    main,
                )
            }
        };
        let status = label("Anwendung wird gestartet");
        status.setFont(Some(&NSFont::boldSystemFontOfSize(19.0)));
        let detail = label("");
        let error = label("");
        error.setTextColor(Some(&NSColor::systemRedColor()));
        error.setAccessibilityLabel(Some(&NSString::from_str("Fehler")));
        let instruction = label("");
        instruction.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let readout = label("");
        readout.setAccessibilityLabel(Some(&NSString::from_str("Aufnahmedauer")));
        let level = NSLevelIndicator::new(main);
        level.setEditable(false);
        level.setMinValue(0.0);
        level.setMaxValue(1000.0);
        level.setAccessibilityLabel(Some(&NSString::from_str("Mikrofonpegel")));
        let progress = NSProgressIndicator::new(main);
        progress.setMinValue(0.0);
        progress.setMaxValue(1.0);
        progress.setAccessibilityLabel(Some(&NSString::from_str("Modelldownload")));
        let scroll = NSScrollView::new(main);
        scroll.setBorderType(NSBorderType::BezelBorder);
        scroll.setBackgroundColor(&NSColor::textBackgroundColor());
        scroll.setDrawsBackground(true);
        scroll.setHasVerticalScroller(true);
        scroll.setHasHorizontalScroller(false);
        scroll.setAutohidesScrollers(true);
        let text = NSTextView::initWithFrame(
            main.alloc(),
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(original.bounds().size.width - 40.0, 200.0),
            ),
        );
        text.setEditable(false);
        text.setSelectable(true);
        text.setRichText(false);
        text.setUsesFontPanel(false);
        text.setFont(Some(&NSFont::systemFontOfSize(15.0)));
        text.setTextContainerInset(NSSize::new(12.0, 12.0));
        text.setVerticallyResizable(true);
        text.setHorizontallyResizable(false);
        text.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
        text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        text.setAccessibilityLabel(Some(&NSString::from_str("Letzter Text")));
        // SAFETY: this text view owns its container; width tracking is standard
        // wrapping, not a replacement layout manager or attributed-text mutation.
        if let Some(container) = unsafe { text.textContainer() } {
            container.setWidthTracksTextView(true);
        }
        scroll.setDocumentView(Some(&text));
        let setup_info = label("");
        setup_info.setMaximumNumberOfLines(3);
        let permission_info = label("");
        let body = WorkspaceBody::new(
            main,
            Views {
                setup_info,
                permission_info,
                status,
                detail,
                error,
                instruction,
                readout,
                level,
                progress,
                bytes: label(""),
                download: button("Modell laden", sel!(download:)),
                cancel_setup: button("Einrichtung abbrechen", sel!(cancelSetup:)),
                settings: button("Berechtigungen prüfen", sel!(settings:)),
                reset: button("Einstellungen zurücksetzen …", sel!(resetSettings:)),
                title: label("Letzter Text"),
                copy: button("Kopieren", sel!(copyText:)),
                clear: button("Löschen", sel!(clearText:)),
                scroll,
                text,
            },
            parent.convertRect_fromView(window.contentLayoutRect(), None),
        );
        body.setTranslatesAutoresizingMaskIntoConstraints(false);
        let constraints = content_constraints(&body, &guide);
        let original_hidden = original.isHidden();
        let responder = window.firstResponder();
        let workspace = Self {
            window,
            original,
            parent,
            _layout_guide: guide,
            constraints,
            original_hidden,
            responder,
            body,
            core,
            _target: target,
            attached: true,
            registered: true,
            _main: PhantomData,
            _owner: owner,
        };
        OWNERS.with(|owners| owners.borrow_mut().insert(key));
        workspace.parent.addSubview(&workspace.body);
        for constraint in &workspace.constraints {
            constraint.setActive(true);
        }
        workspace.window.layoutIfNeeded();
        workspace.original.setHidden(true);
        workspace.core.active.set(true);
        workspace.apply(&WorkspaceState::default());
        // Move focus off a now-hidden webview only when it previously owned it.
        if workspace
            .responder
            .as_ref()
            .is_some_and(|value| responder_in(value, &workspace.original))
        {
            let _accepted = workspace
                .window
                .makeFirstResponder(Some(&workspace.body.ivars().text));
        }
        Ok(workspace)
    }
    fn owns_hierarchy(&self) -> bool {
        parent_of(&self.original).is_some_and(|value| std::ptr::eq(&*value, &*self.parent))
            && parent_of(&self.body).is_some_and(|value| std::ptr::eq(&*value, &*self.parent))
            && self
                .constraints
                .iter()
                .all(|constraint| constraint.isActive())
    }
    fn apply(&self, state: &WorkspaceState) {
        let v = self.body.ivars();
        let recovery = state.recovery.as_ref();
        let status = if recovery.is_some() {
            "Einstellungen können nicht geladen werden"
        } else if state.status.is_empty() {
            "Anwendung wird gestartet"
        } else {
            &state.status
        };
        v.status.setStringValue(&NSString::from_str(status));
        let detail = recovery.map_or(state.detail.as_str(), String::as_str);
        v.detail.setStringValue(&NSString::from_str(detail));
        v.detail.setToolTip(Some(&NSString::from_str(detail)));
        v.detail.setHidden(detail.is_empty());
        v.error.setStringValue(&NSString::from_str(
            state.error.as_deref().unwrap_or_default(),
        ));
        v.error
            .setToolTip(state.error.as_deref().map(NSString::from_str).as_deref());
        v.error
            .setHidden(state.error.as_ref().is_none_or(String::is_empty));
        let hotkey = if state.hotkey.is_empty() {
            "das eingestellte Kürzel"
        } else {
            &state.hotkey
        };
        let instruction = if recovery.is_some() {
            "Aufnahme und Einrichtung bleiben bis zur Wiederherstellung gesperrt. Geladene Sprachmodelle bleiben erhalten.".into()
        } else {
            format!(
                "Im Zieltextfeld: {hotkey} startet und stoppt. Esc bricht ab.\nAufnahme im Appfenster: Dein Text erscheint hier."
            )
        };
        v.instruction
            .setStringValue(&NSString::from_str(&instruction));
        v.instruction
            .setToolTip(Some(&NSString::from_str(&instruction)));
        for (label, content) in [
            (&v.setup_info, &state.setup_info),
            (&v.permission_info, &state.permission_info),
        ] {
            let text = content.as_deref().unwrap_or_default();
            label.setStringValue(&NSString::from_str(text));
            label.setToolTip(Some(&NSString::from_str(text)));
            label.setHidden(text.is_empty());
        }
        v.readout.setHidden(state.recording.is_none());
        v.level.setHidden(state.recording.is_none());
        if let Some(recording) = &state.recording {
            v.readout.setStringValue(&NSString::from_str(&format!(
                "{}:{:02}",
                recording.elapsed_seconds / 60,
                recording.elapsed_seconds % 60
            )));
            v.level.setDoubleValue(f64::from(recording.level_permille));
        }
        v.progress.setHidden(state.download.is_none());
        v.bytes.setHidden(state.download.is_none());
        // SAFETY: these public animation actions run on the main-thread owner.
        unsafe {
            v.progress.stopAnimation(None);
        }
        if let Some(download) = &state.download {
            v.progress.setIndeterminate(download.total.is_none());
            if let Some(total) = download.total {
                v.progress
                    .setDoubleValue(download.received as f64 / total as f64);
            } else {
                unsafe {
                    v.progress.startAnimation(None);
                }
            }
            let bytes = match download.total {
                Some(total) => format!("{} / {} Bytes", download.received, total),
                None => format!("{} Bytes geladen", download.received),
            };
            v.bytes.setStringValue(&NSString::from_str(&bytes));
        }
        for (button, action) in [
            (&v.copy, WorkspaceAction::Copy),
            (&v.clear, WorkspaceAction::Clear),
            (&v.download, WorkspaceAction::Download),
            (&v.cancel_setup, WorkspaceAction::CancelSetup),
            (&v.settings, WorkspaceAction::Settings),
            (&v.reset, WorkspaceAction::ResetSettingsConfirmed),
        ] {
            let enabled = state.allows(action);
            button.setEnabled(enabled);
            if !matches!(action, WorkspaceAction::Copy | WorkspaceAction::Clear) {
                button.setHidden(!enabled);
            }
        }
        if self.core.state.borrow().transcript != state.transcript {
            v.text.setString(&NSString::from_str(&state.transcript));
        }
        self.body.arrange();
    }
    pub fn update(&mut self, state: WorkspaceState) -> Result<(), WorkspaceError> {
        state.validate()?;
        if !self.attached || !self.core.active.get() {
            return Err(WorkspaceError::Inactive);
        }
        if !self.owns_hierarchy() {
            self.core.active.set(false);
            self.body.disable();
            return Err(WorkspaceError::Changed);
        }
        self.apply(&state);
        self.core.state.replace(state);
        Ok(())
    }
    pub fn detach(&mut self) -> Result<(), WorkspaceError> {
        self.core.active.set(false);
        self.body.disable();
        if !self.attached {
            return Ok(());
        }
        let owns = self.owns_hierarchy();
        let restore_focus = self
            .window
            .firstResponder()
            .is_some_and(|value| responder_in(&value, &self.body));
        for constraint in &self.constraints {
            constraint.setActive(false);
        }
        self.body.removeFromSuperview();
        if parent_of(&self.original).is_some_and(|value| std::ptr::eq(&*value, &*self.parent)) {
            if self.original.isHidden() {
                self.original.setHidden(self.original_hidden);
            }
            if restore_focus {
                let _accepted = self.window.makeFirstResponder(self.responder.as_deref());
            }
        }
        self.attached = false;
        self.release();
        if owns {
            Ok(())
        } else {
            Err(WorkspaceError::Changed)
        }
    }
    fn release(&mut self) {
        if self.registered {
            OWNERS.with(|owners| {
                owners
                    .borrow_mut()
                    .remove(&(Retained::as_ptr(&self.window) as usize))
            });
            self.registered = false;
        }
    }
}
fn content_constraints(body: &NSView, guide: &AnyObject) -> [Retained<NSLayoutConstraint>; 4] {
    // SAFETY: NSWindow's public contentLayoutGuide exposes these public layout
    // anchors. Its documented id type is intentionally not downcast to an
    // assumed private class. Retained anchors remain valid during constraint
    // construction; the owner retains both the guide and all four constraints.
    let (leading, trailing, top, bottom): (
        Retained<NSLayoutXAxisAnchor>,
        Retained<NSLayoutXAxisAnchor>,
        Retained<NSLayoutYAxisAnchor>,
        Retained<NSLayoutYAxisAnchor>,
    ) = unsafe {
        (
            msg_send![guide, leadingAnchor],
            msg_send![guide, trailingAnchor],
            msg_send![guide, topAnchor],
            msg_send![guide, bottomAnchor],
        )
    };
    [
        body.leadingAnchor().constraintEqualToAnchor(&leading),
        body.trailingAnchor().constraintEqualToAnchor(&trailing),
        body.topAnchor().constraintEqualToAnchor(&top),
        body.bottomAnchor().constraintEqualToAnchor(&bottom),
    ]
}
fn parent_of(view: &NSView) -> Option<Retained<NSView>> {
    // SAFETY: the main-thread owner retains this view while querying its public
    // hierarchy. The returned parent is retained; no private view class is used.
    unsafe { view.superview() }
}
fn responder_in(responder: &NSResponder, parent: &NSView) -> bool {
    responder
        .downcast_ref::<NSView>()
        .is_some_and(|view| view.isDescendantOf(parent))
}
impl<O: HasWindowHandle + 'static> Drop for NativeWorkspace<O> {
    fn drop(&mut self) {
        if self.detach().is_err() {
            eprintln!("Native workspace hierarchy changed during cleanup");
        }
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    fn core() -> (Core, mpsc::Receiver<WorkspaceAction>) {
        let (send, receive) = mpsc::channel();
        (
            Core {
                state: RefCell::new(WorkspaceState::default()),
                active: Cell::new(true),
                callback: Box::new(move |action| {
                    assert!(send.send(action).is_ok());
                }),
            },
            receive,
        )
    }
    #[test]
    fn reset_requires_recovery_and_explicit_confirmation() -> Result<(), Box<dyn std::error::Error>>
    {
        let (core, receive) = core();
        let asked = Cell::new(false);
        core.reset_with_confirmation(|| {
            asked.set(true);
            true
        });
        assert!(
            !asked.get(),
            "no recovery must not even open a confirmation"
        );
        core.state.borrow_mut().recovery = Some("invalid preferences".into());
        core.reset_with_confirmation(|| false);
        assert!(
            receive.try_recv().is_err(),
            "cancel must never enqueue reset"
        );
        core.reset_with_confirmation(|| true);
        assert_eq!(receive.try_recv()?, WorkspaceAction::ResetSettingsConfirmed);
        Ok(())
    }
    #[test]
    fn a_changed_recovery_or_detached_owner_cannot_confirm_an_old_reset() {
        let (core, receive) = core();
        core.state.borrow_mut().recovery = Some("first".into());
        core.reset_with_confirmation(|| {
            core.state.borrow_mut().recovery = Some("second".into());
            true
        });
        assert!(receive.try_recv().is_err());
        core.reset_with_confirmation(|| {
            core.active.set(false);
            true
        });
        assert!(receive.try_recv().is_err());
    }
    #[test]
    fn recovery_cannot_invoke_normal_actions_even_with_stale_enabled_flags() {
        let (core, receive) = core();
        core.state.replace(WorkspaceState {
            recovery: Some("repair".into()),
            transcript: "old".into(),
            can_copy: true,
            can_clear: true,
            can_download: true,
            can_cancel_setup: true,
            can_settings: true,
            ..WorkspaceState::default()
        });
        for action in [
            WorkspaceAction::Copy,
            WorkspaceAction::Clear,
            WorkspaceAction::Download,
            WorkspaceAction::CancelSetup,
            WorkspaceAction::Settings,
        ] {
            core.dispatch(action);
        }
        assert!(receive.try_recv().is_err());
    }
}
