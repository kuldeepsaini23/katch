//! macOS app events Katch reacts to: activating the app (a click on its Dock icon, ⌘-Tab) shows
//! the window again, and quitting (⌘Q, the Dock, logging out) pauses and saves downloads first.
//!
//! winit owns the application's delegate and panics at start ("tried to get a delegate that was
//! not the one Winit has registered") if anything replaces it, so Katch listens to AppKit's
//! notifications instead: an observer added to the default notification center sees them without
//! touching the delegate.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplicationDidBecomeActiveNotification, NSApplicationWillTerminateNotification};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol};
use std::sync::OnceLock;

/// Run when the app is about to quit: pauses downloads and saves (blocking).
static ON_QUIT: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// When listening began: the activation macOS sends as the app starts isn't a click on the Dock,
/// and must not open a window that started hidden (at login).
static STARTED: OnceLock<std::time::Instant> = OnceLock::new();

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KatchAppObserver"]
    struct Observer;

    unsafe impl NSObjectProtocol for Observer {}

    impl Observer {
        /// The app became active (its Dock icon clicked, ⌘-Tab): bring the hidden window back, as
        /// the menu bar's Open Katch does.
        #[unsafe(method(didBecomeActive:))]
        fn did_become_active(&self, _note: &NSNotification) {
            if STARTED.get().is_some_and(|t| t.elapsed() > std::time::Duration::from_secs(3)) {
                crate::tray::open_window();
            }
        }

        /// The app is quitting: AppKit ends the process right after this returns, so save now.
        #[unsafe(method(willTerminate:))]
        fn will_terminate(&self, _note: &NSNotification) {
            if let Some(quit) = ON_QUIT.get() {
                quit();
            }
        }
    }
);

impl Observer {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // SAFETY: NSObject's plain `init` on a freshly allocated instance.
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

/// Starts listening. Main thread only, once the app is running (it's made with the menu bar
/// icon); `on_quit` runs before the app ends. Leaves winit's application delegate alone.
pub fn install(on_quit: impl Fn() + Send + Sync + 'static) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if ON_QUIT.set(Box::new(on_quit)).is_err() {
        return; // already installed
    }
    let _ = STARTED.set(std::time::Instant::now());
    let observer = Observer::new(mtm);
    let center = NSNotificationCenter::defaultCenter();
    let target: &AnyObject = &observer;
    // SAFETY: the observer implements both selectors with the `(NSNotification)` signature
    // AppKit calls them with, and it lives as long as the app (forgotten below), so the center
    // never calls a freed object.
    unsafe {
        center.addObserver_selector_name_object(target, sel!(didBecomeActive:), Some(NSApplicationDidBecomeActiveNotification), None);
        center.addObserver_selector_name_object(target, sel!(willTerminate:), Some(NSApplicationWillTerminateNotification), None);
    }
    // The notification center doesn't retain its observers: this one lives as long as the app.
    std::mem::forget(observer);
}
