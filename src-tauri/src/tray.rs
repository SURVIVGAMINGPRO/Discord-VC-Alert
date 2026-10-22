//! Menu-bar (macOS) / notification-area (Windows) icon with a neutral look:
//! idle, watching (dot), or quiet (slash).

use crate::state::state;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const ID: &str = "main-tray";

#[cfg(target_os = "macos")]
mod icons {
    pub const IDLE: &[u8] = include_bytes!("../icons/tray/mac-idle.png");
    pub const WATCHING: &[u8] = include_bytes!("../icons/tray/mac-watching.png");
    pub const QUIET: &[u8] = include_bytes!("../icons/tray/mac-quiet.png");
}
#[cfg(not(target_os = "macos"))]
mod icons {
    pub const IDLE: &[u8] = include_bytes!("../icons/tray/win-idle.png");
    pub const WATCHING: &[u8] = include_bytes!("../icons/tray/win-watching.png");
    pub const QUIET: &[u8] = include_bytes!("../icons/tray/win-quiet.png");
}

pub struct TrayItems {
    toggle: MenuItem<Wry>,
    quiet: CheckMenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Start watching", true, None::<&str>)?;
    let quiet = CheckMenuItem::with_id(app, "quiet", "Quiet mode", true, false, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show VC Alert", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit VC Alert", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &quiet, &sep1, &show, &settings, &sep2, &quit])?;
    TrayIconBuilder::with_id(ID)
        .icon(Image::from_bytes(icons::IDLE)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("VC Alert")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            // Menu events run on the main thread; do the work on the async
            // runtime so the main thread never waits on our locks.
            "toggle" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let s = state(&app);
                    let watching = s.run.lock().unwrap().watching;
                    if watching {
                        s.stop();
                    } else if let Err(e) = s.start() {
                        crate::commands::show_main(&app);
                        if e == "needs-credentials" {
                            crate::commands::open_settings_window(&app, Some("connection".into()));
                        } else {
                            s.toast("error", &e);
                        }
                    }
                });
            }
            "quiet" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { crate::commands::toggle_quiet_inner(&app) });
            }
            "show" => crate::commands::show_main(app),
            "settings" => open_settings_later(app, None),
            "quit" => crate::commands::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                if !cfg!(target_os = "macos") {
                    crate::commands::show_main(tray.app_handle());
                }
            }
        })
        .build(app)?;
    app.manage(TrayItems { toggle, quiet });
    Ok(())
}

/// Menu events run on the main thread, and creating a window there deadlocks
/// on Windows, so the Settings window is opened from the async runtime.
fn open_settings_later(app: &AppHandle, tab: Option<String>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { crate::commands::open_settings_window(&app, tab) });
}

pub fn refresh(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let s = state(app);
    let (watching, stopping) = {
        let r = s.run.lock().unwrap();
        (r.watching, r.stopping)
    };
    let quiet = s.cfg().general.quiet;
    let _ = items.toggle.set_text(if stopping {
        "Stopping…"
    } else if watching {
        "Stop watching"
    } else {
        "Start watching"
    });
    let _ = items.toggle.set_enabled(!stopping);
    let _ = items.quiet.set_checked(quiet);
    if let Some(tray) = app.tray_by_id(ID) {
        let bytes = if quiet {
            icons::QUIET
        } else if watching {
            icons::WATCHING
        } else {
            icons::IDLE
        };
        if let Ok(img) = Image::from_bytes(bytes) {
            let _ = tray.set_icon(Some(img));
            #[cfg(target_os = "macos")]
            let _ = tray.set_icon_as_template(true);
        }
    }
}
