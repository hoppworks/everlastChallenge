//! Public AppKit glass around an existing complete content view. No visual or
//! accessibility conformance is implied by successful attachment alone.
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSGlassEffectView, NSGlassEffectViewStyle, NSResponder, NSView,
    NSWindow,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::RefCell, collections::BTreeSet, marker::PhantomData, rc::Rc};

thread_local! {
    // Only current owners are recorded; retained NSWindows keep identities
    // stable, and detach/Drop remove entries. No native objects live here.
    static ATTACHED_WINDOWS: RefCell<BTreeSet<usize>> = const { RefCell::new(BTreeSet::new()) };
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GlassError {
    #[error("Glass attachment requires the native main thread")]
    MainThread,
    #[error("The native window handle is unavailable")]
    Handle,
    #[error("Only AppKit window handles are supported")]
    UnsupportedHandle,
    #[error("The content view is not installed in a window")]
    Detached,
    #[error("A glass content view is already installed")]
    Busy,
    #[error("A content view controller must retain control of its own hierarchy")]
    ManagedContent,
    #[error("The native content hierarchy changed")]
    Changed,
    #[error("The original native responder could not be preserved")]
    Responder,
}

fn same_view(left: &NSView, right: &NSView) -> bool {
    std::ptr::eq(left, right)
}

/// Owns the framework window owner and all native references. The borrowed
/// WindowHandle is consumed only while retaining those references: no lifetime
/// extension, self-reference, raw pointer escape, activation, or theme override.
/// Keep this !Send/!Sync guard on main and explicitly detach before app cleanup.
///
/// ```compile_fail
/// use macos_integration::glass::GlassAttachment;
/// use raw_window_handle::HasWindowHandle;
/// fn requires_send<T: Send>() {}
/// fn check<W: HasWindowHandle + Send + Sync + 'static>() {
///     requires_send::<GlassAttachment<W>>();
/// }
/// ```
///
/// ```compile_fail
/// use macos_integration::glass::GlassAttachment;
/// use raw_window_handle::HasWindowHandle;
/// fn requires_sync<T: Sync>() {}
/// fn check<W: HasWindowHandle + Send + Sync + 'static>() {
///     requires_sync::<GlassAttachment<W>>();
/// }
/// ```
pub struct GlassAttachment<W: HasWindowHandle + 'static> {
    window: Retained<NSWindow>,
    original: Retained<NSView>,
    glass: Retained<NSGlassEffectView>,
    original_autoresizing: NSAutoresizingMaskOptions,
    attached: bool,
    registered: bool,
    _main_thread: PhantomData<Rc<()>>,
    _owner: W,
}

impl<W: HasWindowHandle + 'static> GlassAttachment<W> {
    pub fn attach(owner: W) -> Result<Self, GlassError> {
        let main = MainThreadMarker::new().ok_or(GlassError::MainThread)?;
        let original = {
            let handle = owner.window_handle().map_err(|_| GlassError::Handle)?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return Err(GlassError::UnsupportedHandle);
            };
            // SAFETY: WindowHandle guarantees a valid NSView pointer for its
            // borrow. We are on main and acquire +1 ownership before the borrow
            // ends; the owner value is retained in this guard as well.
            unsafe { Retained::<NSView>::retain(handle.ns_view.as_ptr().cast()) }
                .ok_or(GlassError::Handle)?
        };
        let window = original.window().ok_or(GlassError::Detached)?;
        if !window
            .contentView()
            .is_some_and(|view| same_view(&view, &original))
        {
            return Err(GlassError::Changed);
        }
        if original.clone().downcast::<NSGlassEffectView>().is_ok() {
            return Err(GlassError::Busy);
        }
        if window.contentViewController().is_some() {
            return Err(GlassError::ManagedContent);
        }
        let original_autoresizing = original.autoresizingMask();
        let glass = NSGlassEffectView::new(main);
        glass.setFrame(original.frame());
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        let window_key = Retained::as_ptr(&window) as usize;
        if !ATTACHED_WINDOWS.with(|windows| windows.borrow_mut().insert(window_key)) {
            return Err(GlassError::Busy);
        }
        let attachment = Self {
            window,
            original,
            glass,
            original_autoresizing,
            attached: true,
            registered: true,
            _main_thread: PhantomData,
            _owner: owner,
        };
        // Replace NSWindow's root BEFORE reparenting the old root. setContentView
        // removes its old content, even if that view was put under the new root.
        let focused = attachment.focus_in_original();
        attachment.window.setContentView(Some(&attachment.glass));
        attachment.glass.setContentView(Some(&attachment.original));
        attachment.original.setFrame(attachment.glass.bounds());
        attachment.original.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        if !attachment.owns_hierarchy() {
            return Err(GlassError::Changed);
        }
        attachment.restore_focus(focused.as_deref())?;
        Ok(attachment)
    }

    fn focus_in_original(&self) -> Option<Retained<NSView>> {
        let view = self.window.firstResponder()?.downcast::<NSView>().ok()?;
        (same_view(&view, &self.original) || view.isDescendantOf(&self.original)).then_some(view)
    }

    fn restore_focus(&self, focused: Option<&NSView>) -> Result<(), GlassError> {
        let Some(focused) = focused else {
            return Ok(());
        };
        let previous: &NSResponder = focused;
        let current = self.window.firstResponder();
        if current
            .as_deref()
            .is_some_and(|value| std::ptr::eq(value, previous))
        {
            return Ok(());
        }
        let fallback: &NSResponder = &self.window;
        if current
            .as_deref()
            .is_some_and(|value| !std::ptr::eq(value, fallback))
            || !focused
                .window()
                .is_some_and(|window| std::ptr::eq(&*window, &*self.window))
            || !(same_view(focused, &self.original) || focused.isDescendantOf(&self.original))
        {
            return Err(GlassError::Responder);
        }
        // setContentView falls back to the same NSWindow as first responder.
        // Restore only the immediately preceding descendant in this window;
        // makeFirstResponder does not order, activate, or make the window key.
        if self.window.makeFirstResponder(Some(previous)) {
            Ok(())
        } else {
            Err(GlassError::Responder)
        }
    }

    fn owns_hierarchy(&self) -> bool {
        self.window
            .contentView()
            .is_some_and(|view| same_view(&view, &self.glass))
            && self
                .glass
                .contentView()
                .is_some_and(|view| same_view(&view, &self.original))
            && self.original.isDescendantOf(&self.glass)
    }

    /// Idempotent. A foreign replacement is never undone or reparented. Frame
    /// dimensions are deliberately not restored from the initial size; NSWindow
    /// installs the original root at the window's CURRENT content size.
    pub fn detach(&mut self) -> Result<(), GlassError> {
        if !self.attached {
            return Ok(());
        }
        if !self.owns_hierarchy() {
            return Err(GlassError::Changed);
        }
        let focused = self.focus_in_original();
        self.glass.setContentView(None);
        self.window.setContentView(Some(&self.original));
        self.original
            .setAutoresizingMask(self.original_autoresizing);
        self.attached = false;
        self.release_ownership();
        self.restore_focus(focused.as_deref())?;
        Ok(())
    }

    fn release_ownership(&mut self) {
        if self.registered {
            let key = Retained::as_ptr(&self.window) as usize;
            let removed = ATTACHED_WINDOWS.with(|windows| windows.borrow_mut().remove(&key));
            debug_assert!(removed);
            self.registered = false;
        }
    }
}

impl<W: HasWindowHandle + 'static> Drop for GlassAttachment<W> {
    fn drop(&mut self) {
        if self.detach().is_err() {
            eprintln!("Glass attachment could not completely restore native state");
        }
        self.release_ownership();
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
    fn worker_attach_fails_before_resolving_native_handles()
    -> Result<(), Box<dyn std::error::Error>> {
        let resolutions = Arc::new(AtomicUsize::new(0));
        let owner = Unavailable(resolutions.clone());
        let error = std::thread::spawn(move || GlassAttachment::attach(owner).err())
            .join()
            .map_err(|_| "worker panicked")?;
        assert_eq!(error, Some(GlassError::MainThread));
        assert_eq!(resolutions.load(Ordering::Relaxed), 0);
        Ok(())
    }
}
