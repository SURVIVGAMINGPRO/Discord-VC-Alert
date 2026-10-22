//! VC Alert desktop app (Tauri 2). The engine lives in `vc-core`; this crate
//! wires it to windows, the tray, the OS keychain and the overlay.

mod commands;
mod editor;
mod overlay;
mod platform;
mod secrets;
mod state;
mod tray;

use state::{AppState, Run, Ui};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use vc_core::discord::Status;

/// Settings live in the OS app-data folder (`%APPDATA%\VC Alert`,
/// `~/Library/Application Support/VC Alert`). On Windows, a file named
/// `portable` next to the .exe keeps everything beside the program instead.
fn data_dir(app: &tauri::AppHandle) -> PathBuf {
    if cfg!(windows) {
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            if dir.join("portable").exists() {
                return dir;
            }
        }
    }
    if let Some(d) = std::env::var_os("VC_ALERT_DATA_DIR") {
        return PathBuf::from(d);
    }
    app.path().data_dir().map(|d| d.join("VC Alert")).unwrap_or_else(|_| PathBuf::from("."))
}

fn open_store(dir: &std::path::Path) -> (vc_core::store::Store, Option<String>) {
    let path = dir.join("settings.json");
    match vc_core::store::Store::open(&path) {
        Ok(s) => (s, None),
        Err(vc_core::store::LoadError::Invalid(msg)) => {
            // Never overwrite a broken file silently: move it aside.
            let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
            let aside = dir.join(format!("settings.broken-{stamp}.json"));
            let _ = std::fs::rename(&path, &aside);
            let store = vc_core::store::Store::open(&path).expect("fresh settings");
            (store, Some(format!("{msg} It was moved aside to {} and VC Alert started fresh.", aside.display())))
        }
    }
}

/// Passed by the login item, so "Start minimized" applies only to automatic
/// launches. Opening VC Alert yourself always shows the window: with the Dock
/// icon hidden and the window tucked into the menu bar, a hidden manual launch
/// looks exactly like a crash.
const AUTOSTART_ARG: &str = "--autostart";

fn launched_at_login(args: &[String]) -> bool {
    args.iter().any(|a| a == AUTOSTART_ARG)
}

/// A `.json` passed on the command line ("Open with" on Windows).
fn json_arg(args: &[String]) -> Option<String> {
    args.iter().skip(1).find(|a| a.to_lowercase().ends_with(".json")).cloned()
}

fn import_file(app: &tauri::AppHandle, path: String) {
    commands::show_main(app);
    let _ = app.emit_to("main", "import-file", path);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if launched_at_login(&args) {
                // The login item ran while VC Alert was already open: stay put.
            } else if let Some(p) = json_arg(&args) {
                import_file(app, p);
            } else {
                commands::show_main(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![AUTOSTART_ARG])))
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = data_dir(&handle);
            std::fs::create_dir_all(&dir)?;
            let (store, notice) = open_store(&dir);
            let avatars = {
                let h = handle.clone();
                vc_core::avatars::AvatarCache::new(
                    &dir.join("avatars"),
                    Arc::new(move |key: String| {
                        let _ = h.emit("avatar", &key);
                        overlay::sync(&h);
                    }),
                    Arc::new(|fut| {
                        tauri::async_runtime::spawn(fut);
                    }),
                )
            };
            let st = Arc::new(AppState {
                handle: handle.clone(),
                secrets: secrets::Secrets::new(&dir),
                bank: vc_core::sound::SoundBank::new(&dir),
                player: vc_core::sound::Player::new(),
                avatars,
                store: Mutex::new(store),
                run: Mutex::new(Run {
                    watching: false,
                    stopping: false,
                    status: Status::Stopped,
                    detail: String::new(),
                    members: Vec::new(),
                    stop: None,
                    snoozes: std::collections::HashMap::new(),
                }),
                ui: Mutex::new(Ui {
                    overlay_preview: false,
                    editor_open: false,
                    flash: None,
                    hover: None,
                    removed: Vec::new(),
                    next_id: 0,
                    activity: VecDeque::new(),
                    pending_import: None,
                    notice,
                    editor: None,
                }),
                data_dir: dir,
                displays: Mutex::new(Vec::new()),
                wake: Arc::new(tokio::sync::Notify::new()),
            });
            app.manage(st.clone());
            let cfg = st.cfg();

            #[cfg(target_os = "macos")]
            if cfg.general.hide_dock_icon {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            let main = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("VC Alert")
                .inner_size(460.0, 720.0)
                .min_inner_size(380.0, 520.0)
                .content_protected(cfg.overlay.hide_from_capture)
                .visible(false)
                .center()
                .build()?;
            platform::follow_active_space(&main);
            let args: Vec<String> = std::env::args().collect();
            if cfg.general.start_minimized && launched_at_login(&args) {
                let _ = main.minimize();
                let _ = main.show();
                if cfg.general.close_to_tray {
                    let _ = main.hide();
                }
            } else {
                let _ = main.show();
                let _ = main.set_focus();
            }

            // Login items registered before the flag existed start without it;
            // re-registering rewrites them with it (a small file write).
            if cfg.general.launch_at_login {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    use tauri_plugin_autostart::ManagerExt;
                    if let Err(e) = h.autolaunch().enable() {
                        eprintln!("could not refresh the login item: {e}");
                    }
                });
            }

            tray::create(&handle)?;
            overlay::create(&handle)?;
            tray::refresh(&handle);

            if let Some(p) = json_arg(&args) {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
                    import_file(&h, p);
                });
            }

            if cfg.general.start_watching_on_launch && !cfg.targets.is_empty() && !cfg.client_id.is_empty() {
                let s = st.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                    if let Err(e) = s.start() {
                        if e != "needs-credentials" {
                            s.toast("error", &e);
                        }
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match (window.label(), event) {
                ("main", WindowEvent::CloseRequested { api, .. }) => {
                    if state::state(app).cfg().general.close_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        commands::quit(app);
                    }
                }
                (commands::SETTINGS, WindowEvent::Destroyed) => {
                    state::state(app).ui.lock().unwrap().overlay_preview = false;
                    overlay::sync(app);
                }
                (editor::PANEL, WindowEvent::CloseRequested { .. }) => editor::close(app),
                (editor::GHOST, WindowEvent::Moved(_)) => editor::moved(app),
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::avatar_data,
            commands::add_target,
            commands::add_many,
            commands::remove_target,
            commands::undo_remove,
            commands::rename_target,
            commands::set_person_sound,
            commands::set_leave,
            commands::list_add,
            commands::list_rename,
            commands::list_delete,
            commands::list_switch,
            commands::test_leave_sound,
            commands::snooze,
            commands::update_settings,
            commands::toggle_quiet,
            commands::complete_setup,
            commands::dismiss_notice,
            commands::save_credentials,
            commands::reveal_secret,
            commands::test_connection,
            commands::reset_login,
            commands::open_developer_portal,
            commands::reveal_data_folder,
            commands::check_update,
            commands::download_update,
            commands::open_release_page,
            commands::start_watching,
            commands::stop_watching,
            commands::test_sound,
            commands::choose_sound_file,
            commands::import_preview,
            commands::import_apply,
            commands::import_cancel,
            commands::export_text,
            commands::export_filename,
            commands::export_file,
            commands::list_backups,
            commands::restore_backup,
            commands::clear_pictures,
            commands::open_settings,
            commands::set_overlay_preview,
            commands::overlay_refresh,
            commands::open_position_editor,
            commands::editor_refresh,
            commands::editor_update,
            commands::editor_snap,
            commands::editor_save,
            commands::editor_cancel,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building VC Alert");

    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => commands::show_main(app),
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        RunEvent::Opened { urls } => {
            if let Some(p) = urls.iter().filter_map(|u| u.to_file_path().ok()).find(|p| {
                p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
            }) {
                import_file(app, p.display().to_string());
            }
        }
        RunEvent::ExitRequested { api, code, .. } => {
            // Closing the last window shouldn't quit when we live in the tray.
            if code.is_none() && state::state(app).cfg().general.close_to_tray {
                api.prevent_exit();
            }
        }
        _ => {}
    });
}
