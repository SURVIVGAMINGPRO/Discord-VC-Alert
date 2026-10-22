//! Commands the UI calls. Each one changes state through the store (which
//! saves at once) and then broadcasts a fresh snapshot.
//!
//! Every command is async, so it runs on the async runtime and never on the
//! main thread. The main thread must stay free: from other threads, Tauri's
//! window and monitor calls wait for it, so if it ever waited on one of our
//! locks the app would freeze (the old Start/Stop spinner). Async also keeps
//! keychain reads, which can show a macOS prompt, off the main thread.

use crate::state::{state, Snapshot, Snooze};
use std::time::{Duration, SystemTime};
use vc_core::discord::Status;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use vc_core::config::{SoundSpec, Target};
use vc_core::transfer::{self, ApplyOptions, ExportOptions, ParseOptions, Preview};

type R<T> = Result<T, String>;

pub const SETTINGS: &str = "settings";
const PORTAL: &str = "https://discord.com/developers/applications";

// ----------------------------------------------------------------- reads --

#[tauri::command]
pub async fn get_state(app: AppHandle) -> Snapshot {
    let s = state(&app);
    s.refresh_displays();
    s.snapshot()
}

#[tauri::command]
pub async fn avatar_data(app: AppHandle, keys: Vec<String>) -> HashMap<String, String> {
    let s = state(&app);
    keys.into_iter().filter_map(|k| s.avatars.data_url(&k).map(|d| (k, d))).collect()
}

// --------------------------------------------------------------- the list --

#[tauri::command]
pub async fn add_target(app: AppHandle, label: String, ident: String) -> R<Target> {
    let s = state(&app);
    if ident.trim().is_empty() {
        return Err("Enter a username or user ID.".into());
    }
    let t = s.store.lock().unwrap().add(&label, &ident)?;
    s.targets_changed();
    s.changed();
    Ok(t)
}

#[derive(Serialize)]
pub struct AddMany {
    added: Vec<Target>,
    skipped: Vec<transfer::Skip>,
}

/// A pasted list: usernames/IDs separated by commas, spaces or new lines.
#[tauri::command]
pub async fn add_many(app: AppHandle, text: String) -> R<AddMany> {
    let s = state(&app);
    let parts: Vec<String> = text
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|p| !p.trim().is_empty())
        .map(str::to_string)
        .collect();
    let (added, skipped) = s.store.lock().unwrap().add_many(&parts)?;
    s.targets_changed();
    s.changed();
    Ok(AddMany { added, skipped: skipped.into_iter().map(|(value, reason)| transfer::Skip { value, reason }).collect() })
}

#[derive(Serialize)]
pub struct Removed {
    id: u64,
    label: String,
}

#[tauri::command]
pub async fn remove_target(app: AppHandle, key: String) -> R<Option<Removed>> {
    let s = state(&app);
    let removed = s.store.lock().unwrap().remove(&key)?;
    let out = removed.map(|(t, i)| {
        let mut ui = s.ui.lock().unwrap();
        ui.next_id += 1;
        let id = ui.next_id;
        let label = t.label.clone();
        ui.removed.push((id, t, i));
        if ui.removed.len() > 20 {
            ui.removed.remove(0);
        }
        Removed { id, label }
    });
    s.targets_changed();
    s.changed();
    Ok(out)
}

#[tauri::command]
pub async fn undo_remove(app: AppHandle, id: u64) -> R<()> {
    let s = state(&app);
    let entry = {
        let mut ui = s.ui.lock().unwrap();
        let pos = ui.removed.iter().position(|(i, _, _)| *i == id);
        pos.map(|p| ui.removed.remove(p))
    };
    if let Some((_, t, i)) = entry {
        s.store.lock().unwrap().insert(t, i)?;
        s.targets_changed();
        s.changed();
    }
    Ok(())
}

#[tauri::command]
pub async fn rename_target(app: AppHandle, key: String, label: String) -> R<()> {
    let s = state(&app);
    s.store.lock().unwrap().rename(&key, &label)?;
    s.changed();
    Ok(())
}

#[tauri::command]
pub async fn set_person_sound(app: AppHandle, key: String, spec: Option<SoundSpec>) -> R<()> {
    let s = state(&app);
    s.store.lock().unwrap().set_sound(&key, spec)?;
    s.changed();
    Ok(())
}

/// Their leave sound: off, or on with `spec` (null: the soft beep made from
/// their join sound; else their own beep or a sound file).
#[tauri::command]
pub async fn set_leave(app: AppHandle, key: String, on: bool, spec: Option<SoundSpec>) -> R<()> {
    let s = state(&app);
    s.store.lock().unwrap().set_leave(&key, on, spec)?;
    s.changed();
    Ok(())
}

// ----------------------------------------------------------------- lists --
// Up to 5 named lists; the active one is watched. Every change tells the
// watcher, which re-reads who's in your call for the new list.

#[tauri::command]
pub async fn list_add(app: AppHandle, name: String) -> R<usize> {
    let s = state(&app);
    let i = s.store.lock().unwrap().add_list(&name)?;
    s.targets_changed();
    s.changed();
    Ok(i)
}

#[tauri::command]
pub async fn list_rename(app: AppHandle, index: usize, name: String) -> R<()> {
    let s = state(&app);
    s.store.lock().unwrap().rename_list(index, &name)?;
    s.changed();
    Ok(())
}

/// Returns the deleted list's name.
#[tauri::command]
pub async fn list_delete(app: AppHandle, index: usize) -> R<String> {
    let s = state(&app);
    let name = s.store.lock().unwrap().delete_list(index)?;
    s.targets_changed();
    s.changed();
    Ok(name)
}

#[tauri::command]
pub async fn list_switch(app: AppHandle, index: usize) -> R<()> {
    let s = state(&app);
    s.store.lock().unwrap().switch_list(index)?;
    s.targets_changed();
    s.changed();
    Ok(())
}

/// Play a person's leave sound (as saved). Returns its length in seconds.
#[tauri::command]
pub async fn test_leave_sound(app: AppHandle, key: String) -> R<f64> {
    let s = state(&app);
    let cfg = s.cfg();
    let clip = s.bank.leave_clip_for(&cfg, &key)?;
    let d = clip.duration;
    s.player.play(clip);
    Ok(d)
}

/// Mute one person's join and leave sounds: `mode` is "hour", "call" (until
/// you leave this channel or stop watching) or "off". Kept in memory only, so
/// quitting VC Alert ends every snooze.
#[tauri::command]
pub async fn snooze(app: AppHandle, key: String, mode: String) -> R<()> {
    let s = state(&app);
    let k = key.to_lowercase();
    {
        let mut run = s.run.lock().unwrap();
        match mode.as_str() {
            "hour" => {
                run.snoozes.insert(k.clone(), Snooze::Until(SystemTime::now() + Duration::from_secs(3600)));
            }
            "call" => {
                if !(run.watching && run.status == Status::WatchingChannel) {
                    return Err("Join a voice channel first.".into());
                }
                let channel = run.detail.clone();
                run.snoozes.insert(k.clone(), Snooze::Call(channel));
            }
            "off" => {
                run.snoozes.remove(&k);
            }
            _ => return Err(format!("unknown snooze: {mode}")),
        }
    }
    if mode == "hour" {
        // Refresh every window when it runs out, so the "Snoozed" tag goes.
        let s2 = s.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3601)).await;
            s2.snoozed(&k);
            s2.changed();
        });
    }
    s.changed();
    Ok(())
}

// --------------------------------------------------------------- settings --

#[tauri::command]
pub async fn update_settings(app: AppHandle, patch: Value) -> R<()> {
    let s = state(&app);
    let before = s.cfg();
    s.store.lock().unwrap().patch(&patch)?;
    let after = s.cfg();
    if before.general.launch_at_login != after.general.launch_at_login {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let r = if after.general.launch_at_login { al.enable() } else { al.disable() };
        if let Err(e) = r {
            s.toast("error", &format!("Couldn't change launch at login: {e}"));
        }
    }
    if before.general.hide_dock_icon != after.general.hide_dock_icon {
        apply_dock_icon(&app, after.general.hide_dock_icon);
    }
    if before.overlay.hide_from_capture != after.overlay.hide_from_capture {
        apply_capture_protection(&app, after.overlay.hide_from_capture);
    }
    s.changed();
    Ok(())
}

pub fn apply_dock_icon(app: &AppHandle, hide: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(if hide {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        });
        show_main(app);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, hide);
}

/// "Hide from screen sharing" covers every VC Alert window, not just the overlay.
pub fn apply_capture_protection(app: &AppHandle, on: bool) {
    for (_, w) in app.webview_windows() {
        let _ = w.set_content_protected(on);
    }
}

#[tauri::command]
pub async fn toggle_quiet(app: AppHandle) -> bool {
    toggle_quiet_inner(&app);
    state(&app).cfg().general.quiet
}

pub fn toggle_quiet_inner(app: &AppHandle) {
    let s = state(app);
    let quiet = s.cfg().general.quiet;
    let result = s.store.lock().unwrap().patch(&json!({ "general": { "quiet": !quiet } }));
    if let Err(e) = result {
        s.toast("error", &e);
    }
    s.changed();
}

#[tauri::command]
pub async fn complete_setup(app: AppHandle) -> R<()> {
    update_settings(app, json!({ "general": { "setup_complete": true } })).await
}

#[tauri::command]
pub async fn dismiss_notice(app: AppHandle) {
    let s = state(&app);
    s.ui.lock().unwrap().notice = None;
    *s.secrets.fallback_note.lock().unwrap() = None;
    s.changed();
}

// ------------------------------------------------------------ connection --

#[tauri::command]
pub async fn save_credentials(app: AppHandle, client_id: String, client_secret: Option<String>) -> R<()> {
    let s = state(&app);
    let id = client_id.trim();
    if !id.is_empty() && !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("The Application ID is a long number. Copy it from the Developer Portal.".into());
    }
    s.store.lock().unwrap().set_client_id(id)?;
    if let Some(secret) = client_secret.map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) {
        s.secrets.set_client_secret(&secret)?;
    }
    s.clear_setup_error();
    s.changed();
    Ok(())
}

#[tauri::command]
pub async fn reveal_secret(app: AppHandle) -> String {
    state(&app).secrets.client_secret()
}

#[tauri::command]
pub async fn test_connection(app: AppHandle) -> R<String> {
    let s = state(&app);
    let creds = s.creds();
    if creds.client_id.is_empty() || creds.client_secret.is_empty() {
        return Err("Add the Application ID and Client Secret first.".into());
    }
    let host: std::sync::Arc<dyn vc_core::discord::Host> = std::sync::Arc::new(crate::state::Bridge(s.clone()));
    let fut = vc_core::discord::test_connection(host, creds, vc_core::discord::http_token_fn(), crate::state::ipc_override());
    match tokio::time::timeout(std::time::Duration::from_secs(120), fut).await {
        Ok(Ok(name)) => {
            s.clear_setup_error();
            s.changed();
            Ok(name)
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Discord didn't answer in time. If a popup appeared inside Discord, approve it and try again.".into()),
    }
}

#[tauri::command]
pub async fn reset_login(app: AppHandle) -> R<()> {
    state(&app)
        .secrets
        .set_token(None)
        .map_err(|e| format!("Could not remove the saved login: {e}"))
}

#[tauri::command]
pub async fn open_developer_portal(app: AppHandle) -> R<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(PORTAL, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reveal_data_folder(app: AppHandle) -> R<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = state(&app).data_dir.clone();
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}

// -------------------------------------------------------------- watching --

#[tauri::command]
pub async fn start_watching(app: AppHandle) -> R<()> {
    state(&app).start()
}

#[tauri::command]
pub async fn stop_watching(app: AppHandle) {
    state(&app).stop();
}

// ----------------------------------------------------------------- sounds --

/// Play a sound (even in Quiet mode) and return its length in seconds, so the
/// Test button can lock itself while it plays.
#[tauri::command]
pub async fn test_sound(app: AppHandle, spec: Option<SoundSpec>) -> R<f64> {
    let s = state(&app);
    let cfg = s.cfg();
    let clip = s.bank.for_spec(&cfg, spec.as_ref())?;
    let d = clip.duration;
    s.player.play(clip);
    Ok(d)
}

/// Check a chosen sound file and copy it into the app's `sounds/` folder, so
/// it can't go missing. Returns the path to store.
#[tauri::command]
pub async fn choose_sound_file(app: AppHandle, path: String) -> R<String> {
    let s = state(&app);
    let p = PathBuf::from(&path);
    let bytes = vc_core::sound::read_capped(&p)?;
    vc_core::sound::probe(&bytes)?;
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("sound.wav");
    transfer::save_sound(&s.data_dir, name, &bytes)
}

// ----------------------------------------------------------- import/export --

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[tauri::command]
pub async fn import_preview(app: AppHandle, path: Option<String>, text: Option<String>, migration: bool) -> R<Preview> {
    let s = state(&app);
    let (text, dir) = match (path, text) {
        (Some(p), _) => {
            let p = PathBuf::from(p);
            let meta = std::fs::metadata(&p).map_err(|e| format!("Couldn't open that file: {e}"))?;
            if meta.len() > transfer::MAX_FILE_BYTES {
                return Err("That file is too big to be a VC Alert list (over 5 MB).".into());
            }
            let t = std::fs::read_to_string(&p).map_err(|e| format!("Couldn't read that file: {e}"))?;
            (t, p.parent().map(Path::to_path_buf))
        }
        (None, Some(t)) => (t, None),
        _ => return Err("Nothing to import.".into()),
    };
    let opts = ParseOptions { source_dir: dir, allow_local_files: migration };
    let parsed = transfer::parse(&text, &opts)?;
    let pv = transfer::preview(&parsed, &s.cfg());
    s.ui.lock().unwrap().pending_import = Some((parsed, migration));
    Ok(pv)
}

#[derive(Serialize)]
pub struct ImportResult {
    added: usize,
    updated: usize,
    removed: usize,
    pictures: usize,
}

#[tauri::command]
pub async fn import_apply(app: AppHandle, options: ApplyOptions) -> R<ImportResult> {
    let s = state(&app);
    let (parsed, migration) = s.ui.lock().unwrap().pending_import.take().ok_or("Nothing to import.")?;
    let cur = s.cfg();
    let applied = transfer::apply(&parsed, &cur, &options, &s.data_dir)?;
    backup_now(&s)?;
    let mut cfg = applied.cfg;
    cfg.general.setup_complete = true;
    s.store.lock().unwrap().replace(cfg)?;
    if let Some(c) = &applied.credentials {
        s.secrets.set_client_secret(&c.client_secret)?;
        if migration {
            if let Some(tok) = parsed.token.clone().filter(|t| t.client_id == c.client_id) {
                s.secrets.set_token(Some(tok))?;
            }
        }
    }
    let pictures = match (&parsed.avatars_dir, migration) {
        (Some(dir), true) => s.avatars.import_from(dir),
        _ => 0,
    };
    s.targets_changed();
    s.changed();
    Ok(ImportResult { added: applied.added, updated: applied.updated, removed: applied.removed, pictures })
}

#[tauri::command]
pub async fn import_cancel(app: AppHandle) {
    state(&app).ui.lock().unwrap().pending_import = None;
}

fn backup_now(s: &crate::state::AppState) -> R<()> {
    let path = s.store.lock().unwrap().path().to_path_buf();
    if path.exists() {
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        transfer::backup(&path, &s.data_dir.join("backups"), &stamp)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn export_text(app: AppHandle, options: ExportOptions) -> R<String> {
    let s = state(&app);
    let secret = options.include_credentials.then(|| s.secrets.client_secret());
    transfer::export(&s.cfg(), &options, secret.as_deref(), &s.data_dir, &today(), vc_core::APP_VERSION)
}

#[tauri::command]
pub async fn export_filename(options: ExportOptions) -> String {
    transfer::default_filename(&options, &today())
}

#[tauri::command]
pub async fn export_file(app: AppHandle, options: ExportOptions, path: String) -> R<()> {
    let text = export_text(app, options).await?;
    vc_core::store::write_atomic(Path::new(&path), text.as_bytes())
}

#[derive(Serialize)]
pub struct BackupInfo {
    name: String,
    when: String,
}

#[tauri::command]
pub async fn list_backups(app: AppHandle) -> Vec<BackupInfo> {
    let dir = state(&app).data_dir.join("backups");
    let mut out: Vec<BackupInfo> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .filter(|n| n.starts_with("settings-") && n.ends_with(".json"))
                .map(|n| {
                    let stamp = n.trim_start_matches("settings-").trim_end_matches(".json");
                    let when = stamp.replacen('_', " ", 1).replace('-', ":");
                    let when = match stamp.split_once('_') {
                        Some((d, t)) => format!("{d} {}", t.replace('-', ":")),
                        None => when,
                    };
                    BackupInfo { name: n, when }
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}

#[tauri::command]
pub async fn restore_backup(app: AppHandle, name: String) -> R<()> {
    let s = state(&app);
    if name.contains('/') || name.contains('\\') || !name.starts_with("settings-") || !name.ends_with(".json") {
        return Err("That isn't a backup.".into());
    }
    let path = s.data_dir.join("backups").join(&name);
    let restored = vc_core::store::Store::open(&path).map_err(|_| "That backup can't be read.".to_string())?;
    backup_now(&s)?;
    s.store.lock().unwrap().replace(restored.cfg)?;
    s.targets_changed();
    s.changed();
    Ok(())
}

#[tauri::command]
pub async fn clear_pictures(app: AppHandle) -> usize {
    let s = state(&app);
    let n = s.avatars.clear();
    s.changed();
    n
}

// ---------------------------------------------------------------- windows --

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn open_settings_window(app: &AppHandle, tab: Option<String>) {
    let tab = tab.unwrap_or_else(|| "sound".into());
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit_to(SETTINGS, "settings-tab", &tab);
        return;
    }
    let protect = state(app).cfg().overlay.hide_from_capture;
    let mut b = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(format!("index.html#{tab}").into()))
        .title("Settings")
        .inner_size(760.0, 620.0)
        .min_inner_size(640.0, 520.0)
        .content_protected(protect);
    if let Some(main) = app.get_webview_window("main") {
        if let (Ok(pos), Ok(scale)) = (main.outer_position(), main.scale_factor()) {
            let p = pos.to_logical::<f64>(scale);
            b = b.position(p.x + 40.0, p.y + 40.0);
        }
    } else {
        b = b.center();
    }
    match b.build() {
        Ok(w) => crate::platform::follow_active_space(&w),
        Err(e) => eprintln!("settings window: {e}"),
    }
}

/// Async so the window is created off the main thread: creating windows from
/// a synchronous command deadlocks on Windows (WebView2).
#[tauri::command]
pub async fn open_settings(app: AppHandle, tab: Option<String>) {
    open_settings_window(&app, tab);
}

#[tauri::command]
pub async fn set_overlay_preview(app: AppHandle, on: bool) {
    let s = state(&app);
    s.ui.lock().unwrap().overlay_preview = on;
    crate::overlay::sync(&app);
}

#[tauri::command]
pub async fn overlay_refresh(app: AppHandle) {
    crate::overlay::sync(&app);
}

#[tauri::command]
pub async fn open_position_editor(app: AppHandle) -> R<()> {
    crate::editor::open(&app)
}

#[tauri::command]
pub async fn editor_refresh(app: AppHandle) {
    crate::editor::emit_model(&app);
}

#[tauri::command]
pub async fn editor_update(app: AppHandle, px: i64, layout: String) {
    crate::editor::update(&app, px, &layout);
}

#[tauri::command]
pub async fn editor_snap(app: AppHandle, corner: String) {
    crate::editor::snap(&app, &corner);
}

#[tauri::command]
pub async fn editor_save(app: AppHandle) -> R<()> {
    crate::editor::save(&app)
}

#[tauri::command]
pub async fn editor_cancel(app: AppHandle) {
    crate::editor::close(&app);
}

pub fn quit(app: &AppHandle) {
    state(app).stop();
    crate::editor::close(app);
    app.exit(0);
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) {
    quit(&app);
}
