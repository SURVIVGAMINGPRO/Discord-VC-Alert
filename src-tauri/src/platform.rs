//! Native window tweaks that Tauri doesn't expose.

use tauri::WebviewWindow;

/// macOS: float above other apps (also over full-screen Spaces), on every
/// Space, out of the window cycle, and never hidden when the app deactivates.
/// Windows: a no-activate tool window, so it never takes focus and stays out
/// of Alt-Tab and the taskbar.
pub fn make_overlay(win: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        let w = win.clone();
        let _ = win.run_on_main_thread(move || {
            let Ok(ptr) = w.ns_window() else { return };
            let ns = ptr as *mut AnyObject;
            if ns.is_null() {
                return;
            }
            // NSWindowCollectionBehavior: CanJoinAllSpaces | Stationary | IgnoresCycle | FullScreenAuxiliary
            let behavior: usize = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8);
            unsafe {
                let _: () = msg_send![ns, setLevel: 25isize]; // NSStatusWindowLevel
                let _: () = msg_send![ns, setCollectionBehavior: behavior];
                let _: () = msg_send![ns, setHidesOnDeactivate: false];
                let _: () = msg_send![ns, setHasShadow: false];
            }
        });
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        if let Ok(hwnd) = win.hwnd() {
            let h = hwnd.0 as _;
            unsafe {
                let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                SetWindowLongPtrW(h, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize);
            }
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = win;
}

/// macOS: opening VC Alert (Dock, menu bar, login) brings the window to the
/// Space you're on instead of switching you to the Space it was last on
/// (NSWindowCollectionBehaviorMoveToActiveSpace). Elsewhere a no-op.
pub fn follow_active_space(win: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        let w = win.clone();
        let _ = win.run_on_main_thread(move || {
            let Ok(ptr) = w.ns_window() else { return };
            let ns = ptr as *mut AnyObject;
            if ns.is_null() {
                return;
            }
            unsafe {
                let _: () = msg_send![ns, setCollectionBehavior: 1usize << 1];
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = win;
}

/// Windows: full-screen apps and other topmost windows can cover the overlay,
/// so toggle topmost off and on (v1 did this every 2 s).
pub fn reassert_topmost(win: &WebviewWindow) {
    #[cfg(windows)]
    {
        let _ = win.set_always_on_top(false);
        let _ = win.set_always_on_top(true);
    }
    #[cfg(not(windows))]
    let _ = win;
}
