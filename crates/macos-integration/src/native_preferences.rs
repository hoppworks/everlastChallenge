//! Native, main-thread-owned preferences. The backend remains authoritative.
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, rc::Retained,
    runtime::AnyObject, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSBackingStoreType, NSBezelStyle, NSButton, NSButtonType,
    NSControlStateValueOff, NSControlStateValueOn, NSPopUpButton, NSTabView, NSTabViewItem,
    NSTextField, NSView, NSWindow, NSWindowDidBecomeKeyNotification, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferencesDraft {
    pub device: Option<String>,
    pub language: String,
    pub key: char,
    pub option: bool,
    pub command: bool,
    pub control: bool,
    pub shift: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceChoice {
    pub value: String,
    pub label: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencesPermission {
    Granted,
    Denied,
    NotDetermined,
    Restricted,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferencesState {
    pub current: PreferencesDraft,
    pub devices: Vec<PreferenceChoice>,
    pub languages: Vec<PreferenceChoice>,
    pub key_labels: Vec<(char, String)>,
    pub busy: bool,
    pub permissions_busy: bool,
    pub microphone: PreferencesPermission,
    pub accessibility: PreferencesPermission,
    pub model_summary: String,
    pub model_details: String,
    pub error: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferencesAction {
    Save(PreferencesDraft),
    RefreshDevices,
    RefreshPermissions,
    RequestMic,
    RequestAX,
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PreferencesError {
    #[error("Native preferences require the main thread")]
    MainThread,
    #[error("The preferences state is invalid or too large")]
    State,
    #[error("The preferences window is detached")]
    Inactive,
}
/// A native window owner. It cannot cross threads.
/// ```compile_fail
/// fn send<T: Send>() {} send::<macos_integration::native_preferences::NativePreferences>();
/// ```
/// ```compile_fail
/// fn sync<T: Sync>() {} sync::<macos_integration::native_preferences::NativePreferences>();
/// ```
pub struct NativePreferences {
    window: Retained<NSWindow>,
    target: Retained<PreferencesTarget>,
    center: Retained<NSNotificationCenter>,
    core: Rc<Core>,
    detached: bool,
    _main: PhantomData<Rc<()>>,
}

struct Controls {
    tabs: Retained<NSTabView>,
    device: Retained<NSPopUpButton>,
    language: Retained<NSPopUpButton>,
    key: Retained<NSPopUpButton>,
    modifiers: [Retained<NSButton>; 4],
    save: Retained<NSButton>,
    refresh: Retained<NSButton>,
    mic: Retained<NSButton>,
    ax: Retained<NSButton>,
    mic_status: Retained<NSTextField>,
    ax_status: Retained<NSTextField>,
    model: Retained<NSTextField>,
    model_details: Retained<NSTextField>,
    disclosure: Retained<NSButton>,
    error: Retained<NSTextField>,
    device_values: Vec<Option<String>>,
    language_values: Vec<String>,
}
struct Core {
    active: Cell<bool>,
    state: RefCell<Option<PreferencesState>>,
    controls: RefCell<Option<Controls>>,
    callback: Box<dyn Fn(PreferencesAction) + Send + Sync>,
}
#[derive(Clone, Copy)]
enum Action {
    Save,
    Refresh,
    Mic,
    Ax,
    Focus,
}
impl Core {
    fn emit(&self, action: PreferencesAction) {
        if self.active.get() && catch_unwind(AssertUnwindSafe(|| (self.callback)(action))).is_err()
        {
            self.active.set(false);
            if let Some(ui) = self.controls.borrow().as_ref() {
                ui.disable();
            }
            eprintln!("Native preferences callback failed; actions disabled");
        }
    }
    fn dispatch(&self, action: Action) {
        if !self.active.get() {
            return;
        }
        let event = {
            let state = self.state.borrow();
            let Some(state) = state.as_ref() else {
                return;
            };
            match action {
                Action::Focus => Some(PreferencesAction::RefreshPermissions),
                Action::Refresh if !state.busy => Some(PreferencesAction::RefreshDevices),
                Action::Mic
                    if !state.permissions_busy
                        && state.microphone == PreferencesPermission::NotDetermined =>
                {
                    Some(PreferencesAction::RequestMic)
                }
                Action::Ax
                    if !state.permissions_busy
                        && matches!(
                            state.accessibility,
                            PreferencesPermission::Denied | PreferencesPermission::NotDetermined
                        ) =>
                {
                    Some(PreferencesAction::RequestAX)
                }
                Action::Save if !state.busy => {
                    let ui = self.controls.borrow();
                    let Some(ui) = ui.as_ref() else {
                        return;
                    };
                    let draft = ui.draft(&state.current);
                    if valid_draft(&draft) && draft != state.current {
                        Some(PreferencesAction::Save(draft))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        if let Some(event) = event {
            self.emit(event);
        }
    }
    fn toggle_details(&self) {
        if !self.active.get() {
            return;
        }
        if let Some(ui) = self.controls.borrow().as_ref() {
            ui.model_details
                .setHidden(ui.disclosure.state() != NSControlStateValueOn);
        }
    }
    fn changed(&self) {
        if !self.active.get() {
            return;
        }
        if let (Some(state), Some(ui)) = (
            self.state.borrow().as_ref(),
            self.controls.borrow().as_ref(),
        ) {
            let draft = ui.draft(&state.current);
            ui.save
                .setEnabled(!state.busy && valid_draft(&draft) && draft != state.current);
            if !valid_draft(&draft) {
                ui.set_error("Wähle mindestens Option, Command oder Control.");
            } else {
                ui.set_error(state.error.as_deref().unwrap_or(""));
            }
        }
    }
}
struct TargetIvars {
    core: Rc<Core>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetIvars]
    struct PreferencesTarget;
    impl PreferencesTarget {
        #[unsafe(method(save:))]
        fn save(&self,_sender:Option<&AnyObject>) { let _keep=self.retain(); self.ivars().core.dispatch(Action::Save); }
        #[unsafe(method(refresh:))]
        fn refresh(&self,_sender:Option<&AnyObject>) { let _keep=self.retain(); self.ivars().core.dispatch(Action::Refresh); }
        #[unsafe(method(microphone:))]
        fn microphone(&self,_sender:Option<&AnyObject>) { let _keep=self.retain(); self.ivars().core.dispatch(Action::Mic); }
        #[unsafe(method(accessibility:))]
        fn accessibility(&self,_sender:Option<&AnyObject>) { let _keep=self.retain(); self.ivars().core.dispatch(Action::Ax); }
        #[unsafe(method(changed:))]
        fn changed(&self,_sender:Option<&AnyObject>) { self.ivars().core.changed(); }
        #[unsafe(method(details:))]
        fn details(&self,_sender:Option<&AnyObject>) { let _keep=self.retain(); self.ivars().core.toggle_details(); }
        #[unsafe(method(focused:))]
        fn focused(&self,_notification:&NSNotification) { let _keep=self.retain(); self.ivars().core.dispatch(Action::Focus); }
    }
);
impl PreferencesTarget {
    fn new(main: MainThreadMarker, core: Rc<Core>) -> Retained<Self> {
        let value = Self::alloc(main).set_ivars(TargetIvars { core });
        // SAFETY: NSObject designated initializer on a new subclass with initialized ivars.
        unsafe { msg_send![super(value), init] }
    }
}
fn valid_draft(draft: &PreferencesDraft) -> bool {
    draft.key.is_ascii_uppercase()
        && (draft.option || draft.command || draft.control)
        && matches!(draft.language.as_str(), "german" | "english")
}
fn bounded(text: &str, max: usize) -> bool {
    text.len() <= max && !text.contains('\0')
}
fn validate(state: &PreferencesState) -> Result<(), PreferencesError> {
    let choices = |items: &[PreferenceChoice], max: usize| {
        let mut keys = BTreeSet::new();
        items.len() <= max
            && items.iter().all(|item| {
                !item.value.is_empty()
                    && bounded(&item.value, 512)
                    && !item.label.is_empty()
                    && bounded(&item.label, 1024)
                    && keys.insert(&item.value)
            })
    };
    let mut keys = BTreeSet::new();
    if !valid_draft(&state.current)
        || state
            .current
            .device
            .as_ref()
            .is_some_and(|v| v.is_empty() || !bounded(v, 512))
        || !choices(&state.devices, 129)
        || state.languages.is_empty()
        || !choices(&state.languages, 2)
        || state
            .languages
            .iter()
            .any(|item| !matches!(item.value.as_str(), "german" | "english"))
        || state.key_labels.len() > 26
        || state.key_labels.iter().any(|(key, label)| {
            !key.is_ascii_uppercase() || !bounded(label, 256) || !keys.insert(*key)
        })
        || !bounded(&state.model_summary, 512)
        || !bounded(&state.model_details, 4096)
        || state.error.as_ref().is_some_and(|v| !bounded(v, 2048))
    {
        return Err(PreferencesError::State);
    }
    Ok(())
}
fn frame(x: f64, top: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(x, 360.0 - top - height),
        NSSize::new(width, height),
    )
}
fn label(
    main: MainThreadMarker,
    page: &NSView,
    text: &str,
    x: f64,
    top: f64,
    width: f64,
    height: f64,
) -> Retained<NSTextField> {
    let value = NSTextField::wrappingLabelWithString(&NSString::from_str(text), main);
    value.setFrame(frame(x, top, width, height));
    page.addSubview(&value);
    value
}
fn status(permission: PreferencesPermission) -> &'static str {
    match permission {
        PreferencesPermission::Granted => "Freigegeben",
        PreferencesPermission::Denied => "Nicht freigegeben",
        PreferencesPermission::NotDetermined => "Noch nicht angefragt",
        PreferencesPermission::Restricted => "Durch System- oder Verwaltungsvorgaben eingeschränkt",
        PreferencesPermission::Unavailable => "Status nicht verfügbar",
    }
}
impl Controls {
    fn set_error(&self, text: &str) {
        self.error.setStringValue(&NSString::from_str(text));
        self.error.setHidden(text.trim().is_empty());
        self.error.setToolTip(
            (!text.trim().is_empty())
                .then(|| NSString::from_str(text))
                .as_deref(),
        );
    }
    fn draft(&self, fallback: &PreferencesDraft) -> PreferencesDraft {
        let index = |index: isize| usize::try_from(index).ok();
        PreferencesDraft {
            device: index(self.device.indexOfSelectedItem())
                .and_then(|i| self.device_values.get(i))
                .cloned()
                .unwrap_or_else(|| fallback.device.clone()),
            language: index(self.language.indexOfSelectedItem())
                .and_then(|i| self.language_values.get(i))
                .cloned()
                .unwrap_or_else(|| fallback.language.clone()),
            key: index(self.key.indexOfSelectedItem())
                .and_then(|i| u8::try_from(i).ok())
                .filter(|i| *i < 26)
                .map(|i| char::from(b'A' + i))
                .unwrap_or(fallback.key),
            option: self.modifiers[0].state() == NSControlStateValueOn,
            command: self.modifiers[1].state() == NSControlStateValueOn,
            control: self.modifiers[2].state() == NSControlStateValueOn,
            shift: self.modifiers[3].state() == NSControlStateValueOn,
        }
    }
    fn disable(&self) {
        for popup in [&self.device, &self.language, &self.key] {
            popup.setEnabled(false);
        }
        for button in self.modifiers.iter().chain([
            &self.save,
            &self.refresh,
            &self.mic,
            &self.ax,
            &self.disclosure,
        ]) {
            button.setEnabled(false);
        }
    }
    fn apply(&mut self, state: &PreferencesState, draft: &PreferencesDraft) {
        self.device.removeAllItems();
        self.device_values.clear();
        self.device
            .addItemWithTitle(&NSString::from_str("Systemstandard"));
        self.device_values.push(None);
        for device in &state.devices {
            self.device
                .addItemWithTitle(&NSString::from_str(&device.label));
            self.device_values.push(Some(device.value.clone()));
            if let Some(item) = self.device.lastItem() {
                item.setEnabled(device.enabled);
            }
        }
        if !self.device_values.contains(&draft.device) {
            self.device.addItemWithTitle(&NSString::from_str(&format!(
                "{} (nicht verfügbar)",
                draft.device.as_deref().unwrap_or("")
            )));
            self.device_values.push(draft.device.clone());
            if let Some(item) = self.device.lastItem() {
                item.setEnabled(false);
            }
        }
        if let Some(index) = self
            .device_values
            .iter()
            .position(|value| value == &draft.device)
        {
            self.device.selectItemAtIndex(index as isize);
        }
        self.language.removeAllItems();
        self.language_values.clear();
        for language in &state.languages {
            self.language
                .addItemWithTitle(&NSString::from_str(&language.label));
            self.language_values.push(language.value.clone());
            if let Some(item) = self.language.lastItem() {
                item.setEnabled(language.enabled);
            }
        }
        if !self.language_values.contains(&draft.language) {
            self.language.addItemWithTitle(&NSString::from_str(
                "Gespeicherte Sprache (nicht verfügbar)",
            ));
            self.language_values.push(draft.language.clone());
            if let Some(item) = self.language.lastItem() {
                item.setEnabled(false);
            }
        }
        if let Some(index) = self
            .language_values
            .iter()
            .position(|value| value == &draft.language)
        {
            self.language.selectItemAtIndex(index as isize);
        }
        self.key.removeAllItems();
        for key in 'A'..='Z' {
            let label = state
                .key_labels
                .iter()
                .find(|(value, _)| *value == key)
                .map(|(_, label)| label.as_str())
                .unwrap_or("Layout unbekannt");
            self.key
                .addItemWithTitle(&NSString::from_str(&format!("{label} (US-Position {key})")));
        }
        self.key
            .selectItemAtIndex(isize::from(draft.key as u8 - b'A'));
        for (button, on) in
            self.modifiers
                .iter()
                .zip([draft.option, draft.command, draft.control, draft.shift])
        {
            button.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            button.setEnabled(!state.busy);
        }
        for popup in [&self.device, &self.language, &self.key] {
            popup.setEnabled(!state.busy);
        }
        self.refresh.setEnabled(!state.busy);
        self.save
            .setEnabled(!state.busy && valid_draft(draft) && draft != &state.current);
        self.mic.setEnabled(
            !state.permissions_busy && state.microphone == PreferencesPermission::NotDetermined,
        );
        self.mic
            .setHidden(state.microphone != PreferencesPermission::NotDetermined);
        self.ax.setEnabled(
            !state.permissions_busy
                && matches!(
                    state.accessibility,
                    PreferencesPermission::NotDetermined | PreferencesPermission::Denied
                ),
        );
        self.ax.setHidden(!matches!(
            state.accessibility,
            PreferencesPermission::NotDetermined | PreferencesPermission::Denied
        ));
        self.mic_status
            .setStringValue(&NSString::from_str(status(state.microphone)));
        self.ax_status
            .setStringValue(&NSString::from_str(status(state.accessibility)));
        self.model
            .setStringValue(&NSString::from_str(&state.model_summary));
        self.model_details
            .setStringValue(&NSString::from_str(&state.model_details));
        self.set_error(state.error.as_deref().unwrap_or(""));
    }
}
impl NativePreferences {
    pub fn new(
        callback: impl Fn(PreferencesAction) + Send + Sync + 'static,
    ) -> Result<Self, PreferencesError> {
        let main = MainThreadMarker::new().ok_or(PreferencesError::MainThread)?;
        // SAFETY: finite, fixed-size owned utility window, constructed only on main.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                main.alloc(),
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(600., 460.)),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: Retained owns lifetime; native Close must not release our owner.
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setTitle(&NSString::from_str("Einstellungen"));
        let root = NSView::initWithFrame(
            main.alloc(),
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(600., 460.)),
        );
        let tabs = NSTabView::initWithFrame(
            main.alloc(),
            NSRect::new(NSPoint::new(16., 48.), NSSize::new(568., 396.)),
        );
        root.addSubview(&tabs);
        let mut pages = Vec::with_capacity(3);
        for title in ["Aufnahme", "Berechtigungen", "Modell"] {
            // SAFETY: nil identifier is documented; items remain retained by NSTabView.
            let item = unsafe { NSTabViewItem::initWithIdentifier(main.alloc(), None) };
            item.setLabel(&NSString::from_str(title));
            let page = NSView::initWithFrame(
                main.alloc(),
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(540., 360.)),
            );
            item.setView(Some(&page));
            tabs.addTabViewItem(&item);
            pages.push(page);
        }
        let core = Rc::new(Core {
            active: Cell::new(true),
            state: RefCell::new(None),
            controls: RefCell::new(None),
            callback: Box::new(callback),
        });
        let target = PreferencesTarget::new(main, core.clone());
        let button = |page: &NSView, title: &str, selector, x, top, width| {
            // SAFETY: target owns each declared one-argument selector and is retained through detach.
            let value = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(title),
                    Some(&target),
                    Some(selector),
                    main,
                )
            };
            value.setFrame(frame(x, top, width, 32.));
            page.addSubview(&value);
            value
        };
        let popup = |page: &NSView, top| {
            let value = NSPopUpButton::initWithFrame_pullsDown(
                main.alloc(),
                frame(144., top, 350., 30.),
                false,
            );
            value.setAutoenablesItems(false);
            // SAFETY: changed: is declared on our retained target; only valid NSControl senders dispatch it.
            unsafe {
                value.setTarget(Some(&target));
                value.setAction(Some(sel!(changed:)));
            }
            page.addSubview(&value);
            value
        };
        let page = &pages[0];
        label(main, page, "Mikrofon", 20., 16., 116., 24.);
        let device = popup(page, 12.);
        device.setAccessibilityLabel(Some(&NSString::from_str("Mikrofon")));
        let refresh = button(
            page,
            "Mikrofone aktualisieren",
            sel!(refresh:),
            144.,
            48.,
            220.,
        );
        label(main, page, "Sprache", 20., 98., 116., 24.);
        let language = popup(page, 94.);
        language.setAccessibilityLabel(Some(&NSString::from_str("Sprache")));
        label(main, page, "Tastenkürzel", 20., 145., 116., 24.);
        let key = popup(page, 141.);
        key.setAccessibilityLabel(Some(&NSString::from_str("Physische Taste (US-Position)")));
        let modifiers = ["Option", "Command", "Control", "Shift"].map(|title| {
            // SAFETY: fixed declared target/action as above.
            unsafe {
                NSButton::checkboxWithTitle_target_action(
                    &NSString::from_str(title),
                    Some(&target),
                    Some(sel!(changed:)),
                    main,
                )
            }
        });
        for (index, control) in modifiers.iter().enumerate() {
            control.setFrame(frame(144. + index as f64 * 88., 180., 88., 28.));
            page.addSubview(control);
        }
        label(
            main,
            page,
            "Die Beschriftung folgt dem Layout. Gespeichert wird die physische US-Position.",
            144.,
            218.,
            350.,
            48.,
        );
        let save = button(page, "Speichern", sel!(save:), 384., 298., 110.);
        let page = &pages[1];
        label(main, page, "Mikrofon", 20., 12., 450., 24.);
        let mic_status = label(main, page, "", 20., 40., 450., 24.);
        label(
            main,
            page,
            "System Settings → Privacy & Security → Microphone",
            20.,
            70.,
            480.,
            40.,
        );
        let mic = button(
            page,
            "Mikrofonzugriff anfragen",
            sel!(microphone:),
            20.,
            112.,
            236.,
        );
        label(
            main,
            page,
            "Bedienungshilfen (Accessibility)",
            20.,
            168.,
            480.,
            24.,
        );
        let ax_status = label(main, page, "", 20., 196., 480., 24.);
        label(
            main,
            page,
            "System Settings → Privacy & Security → Device Control and Data Access (Accessibility)",
            20.,
            226.,
            480.,
            48.,
        );
        let ax = button(
            page,
            "Freigabe anfragen",
            sel!(accessibility:),
            20.,
            282.,
            200.,
        );
        let page = &pages[2];
        let model = label(main, page, "", 20., 16., 480., 56.);
        label(
            main,
            page,
            "Die Spracherkennung erfolgt lokal auf diesem Mac.",
            20.,
            80.,
            480.,
            40.,
        );
        let disclosure = button(page, "", sel!(details:), 20., 128., 20.);
        disclosure.setButtonType(NSButtonType::PushOnPushOff);
        disclosure.setBezelStyle(NSBezelStyle::Disclosure);
        disclosure.setState(NSControlStateValueOff);
        disclosure.setAccessibilityLabel(Some(&NSString::from_str("Technische Details")));
        label(main, page, "Technische Details", 48., 132., 440., 24.);
        let model_details = label(main, page, "", 20., 172., 480., 170.);
        model_details.setHidden(true);
        let error = NSTextField::wrappingLabelWithString(&NSString::new(), main);
        error.setAccessibilityLabel(Some(&NSString::from_str("Hinweis")));
        error.setFrame(NSRect::new(NSPoint::new(24., 8.), NSSize::new(552., 36.)));
        root.addSubview(&error);
        window.setContentView(Some(&root));
        // Permission panes can contain no focusable form controls. Give the
        // native tab view an initial keyboard target without overriding the
        // user's system-wide Keyboard navigation setting.
        window.setInitialFirstResponder(Some(&tabs));
        window.center();
        core.controls.replace(Some(Controls {
            tabs,
            device,
            language,
            key,
            modifiers,
            save,
            refresh,
            mic,
            ax,
            mic_status,
            ax_status,
            model,
            model_details,
            disclosure,
            error,
            device_values: Vec::new(),
            language_values: Vec::new(),
        }));
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: scoped to our retained NSWindow, delivered on its main thread;
        // target is retained and removed from the same center before owner release.
        unsafe {
            center.addObserver_selector_name_object(
                &target,
                sel!(focused:),
                Some(NSWindowDidBecomeKeyNotification),
                Some(&window),
            );
        }
        Ok(Self {
            window,
            target,
            center,
            core,
            detached: false,
            _main: PhantomData,
        })
    }
    pub fn update(&mut self, state: PreferencesState) -> Result<(), PreferencesError> {
        if !self.core.active.get() {
            return Err(PreferencesError::Inactive);
        }
        validate(&state)?;
        let old = self.core.state.borrow();
        if old.as_ref() == Some(&state) {
            return Ok(());
        }
        let controls = self.core.controls.borrow();
        let draft = match (old.as_ref(), controls.as_ref()) {
            (Some(previous), Some(ui)) if previous.current == state.current => {
                ui.draft(&previous.current)
            }
            _ => state.current.clone(),
        };
        let initial = old.is_none();
        drop(controls);
        drop(old);
        if let Some(ui) = self.core.controls.borrow_mut().as_mut() {
            ui.apply(&state, &draft);
            if initial {
                ui.tabs.selectTabViewItemAtIndex(
                    if state.microphone != PreferencesPermission::Granted
                        || state.accessibility != PreferencesPermission::Granted
                    {
                        1
                    } else {
                        0
                    },
                );
            }
        }
        self.core.state.replace(Some(state));
        Ok(())
    }
    pub fn show(&self) -> Result<(), PreferencesError> {
        if !self.core.active.get() {
            return Err(PreferencesError::Inactive);
        }
        self.core.dispatch(Action::Focus);
        let main = MainThreadMarker::new().ok_or(PreferencesError::MainThread)?;
        NSApplication::sharedApplication(main).activate();
        self.window.makeKeyAndOrderFront(None);
        // Native key-view eligibility can exclude all form controls when full
        // keyboard access is off. Supply the explicitly configured entry point
        // only if AppKit has no current control responder; preserve active edits.
        if self
            .window
            .firstResponder()
            .is_none_or(|responder| responder.downcast_ref::<NSWindow>() == Some(&*self.window))
            && let Some(initial) = self.window.initialFirstResponder()
        {
            self.window.makeFirstResponder(Some(&initial));
        }
        Ok(())
    }
    /// Hide while retaining native draft and selected tab. Traffic-light Close and
    /// the standard app menu's Command-W use normal NSWindow close behavior.
    pub fn close(&self) -> Result<(), PreferencesError> {
        if !self.core.active.get() {
            return Err(PreferencesError::Inactive);
        }
        self.window.orderOut(None);
        Ok(())
    }
    pub fn detach(&mut self) -> Result<(), PreferencesError> {
        if self.detached {
            return Ok(());
        }
        self.detached = true;
        self.core.active.set(false);
        // SAFETY: unregister only our retained target and exact owned window.
        unsafe {
            self.center.removeObserver_name_object(
                &self.target,
                Some(NSWindowDidBecomeKeyNotification),
                Some(&self.window),
            );
        }
        if let Some(ui) = self.core.controls.borrow().as_ref() {
            ui.disable();
        }
        self.window.orderOut(None);
        self.window.close();
        Ok(())
    }
}
impl Drop for NativePreferences {
    fn drop(&mut self) {
        if self.detach().is_err() {
            eprintln!("Native preferences cleanup failed");
        }
    }
}
