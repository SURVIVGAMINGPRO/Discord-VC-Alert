//! App state shared by every window, the watcher bridge, and the snapshot
//! that the UI renders from. The Rust side is the single source of truth: the
//! UI calls commands, and every change is broadcast as a `state` event.

use crate::secrets::Secrets;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use vc_core::avatars::{AvatarCache, UserRef};
use vc_core::config::{Config, Target};
use vc_core::discord::{Creds, Host, Member, Status, Token};
use vc_core::sound::{Player, SoundBank};
use vc_core::store::Store;
use vc_core::transfer::Parsed;

#[derive(Debug, Clone, Serialize)]
pub struct Activity {
    pub at: String,
    pub key: String,
    pub label: String,
    pub what: &'static str,
}

pub struct Run {
    pub watching: bool,
    pub stopping: bool,
    pub status: Status,
    pub detail: String,
    pub members: Vec<Member>,
    pub stop: Option<CancellationToken>,
}

pub struct Ui {
    pub overlay_preview: bool,
    pub editor_open: bool,
    pub flash: Option<(String, Instant)>,
    pub hover: Option<usize>,
    pub removed: Vec<(u64, Target, usize)>,
    pub next_id: u64,
    pub activity: VecDeque<Activity>,
    pub pending_import: Option<(Parsed, bool)>,
    pub notice: Option<String>,
    pub editor: Option<crate::editor::Editor>,
}

pub struct AppState {
    pub handle: AppHandle,
    pub data_dir: PathBuf,
    pub store: Mutex<Store>,
    pub secrets: Secrets,
    pub bank: SoundBank,
    pub player: Player,
    pub avatars: Arc<AvatarCache>,
    pub run: Mutex<Run>,
    pub ui: Mutex<Ui>,
    /// The display list for the overlay picker. Refreshed when the UI asks for
    /// state, not on every change (each read is a main-thread round trip).
    pub displays: Mutex<Vec<DisplayInfo>>,
}

#[derive(Clone, Serialize)]
pub struct DisplayInfo {
    pub name: String,
    pub label: String,
    pub primary: bool,
}

#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub config: Config,
    pub has_secret: bool,
    pub watching: bool,
    pub stopping: bool,
    pub status: Status,
    pub detail: String,
    pub members: Vec<Member>,
    pub activity: Vec<Activity>,
    pub platform: &'static str,
    pub version: &'static str,
    pub data_dir: String,
    pub displays: Vec<DisplayInfo>,
    pub notice: Option<String>,
    pub editor_open: bool,
}

pub const PLATFORM: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(windows) {
    "windows"
} else {
    "linux"
};

impl AppState {
    pub fn cfg(&self) -> Config {
        self.store.lock().unwrap().cfg.clone()
    }

    pub fn snapshot(&self) -> Snapshot {
        // Never hold a lock while calling into Tauri: from a background thread,
        // monitor and window getters wait for the main thread, and if the main
        // thread is waiting for that same lock, both freeze (the spinning
        // cursor on Start/Stop). So everything that may touch the main thread
        // or the keychain happens first, and the locks are only held to copy.
        let displays = self.displays.lock().unwrap().clone();
        let has_secret = self.secrets.has_secret();
        let fallback = self.secrets.fallback_note.lock().unwrap().clone();
        let config = self.cfg();
        // Lock order everywhere: ui, then run.
        let ui = self.ui.lock().unwrap();
        let run = self.run.lock().unwrap();
        let notice = ui.notice.clone().or(fallback);
        Snapshot {
            config,
            has_secret,
            watching: run.watching,
            stopping: run.stopping,
            status: run.status,
            detail: run.detail.clone(),
            members: if run.watching { run.members.clone() } else { Vec::new() },
            activity: ui.activity.iter().cloned().collect(),
            platform: PLATFORM,
            version: vc_core::APP_VERSION,
            data_dir: self.data_dir.display().to_string(),
            displays,
            notice,
            editor_open: ui.editor_open,
        }
    }

    /// Re-read the displays (two main-thread round trips). Never call this
    /// while holding a lock.
    pub fn refresh_displays(&self) {
        let list = self.read_displays();
        *self.displays.lock().unwrap() = list;
    }

    fn read_displays(&self) -> Vec<DisplayInfo> {
        let primary = self.handle.primary_monitor().ok().flatten();
        self.handle
            .available_monitors()
            .unwrap_or_default()
            .into_iter()
            .map(|m| {
                let name = m.name().cloned().unwrap_or_default();
                let size = m.size().to_logical::<f64>(m.scale_factor());
                let is_primary = primary.as_ref().is_some_and(|p| p.name() == m.name() && p.position() == m.position());
                DisplayInfo {
                    label: format!("{} ({} × {})", if name.is_empty() { "Display" } else { &name }, size.width.round(), size.height.round()),
                    name,
                    primary: is_primary,
                }
            })
            .collect()
    }

    /// After a setup problem, a successful connection test or new credentials
    /// clear it, so the main button goes back to "Start watching".
    pub fn clear_setup_error(&self) {
        let mut run = self.run.lock().unwrap();
        if !run.watching && run.status == Status::SetupError {
            run.status = Status::Stopped;
            run.detail.clear();
        }
    }

    /// Tell every window (and the tray and overlay) that something changed.
    pub fn changed(&self) {
        let _ = self.handle.emit("state", self.snapshot());
        crate::tray::refresh(&self.handle);
        crate::overlay::sync(&self.handle);
    }

    pub fn toast(&self, kind: &str, message: &str) {
        let _ = self.handle.emit_to("main", "toast", serde_json::json!({ "kind": kind, "message": message }));
    }

    pub fn label_of(&self, key: &str) -> String {
        self.store.lock().unwrap().cfg.find(key).map(|t| t.label.clone()).unwrap_or_else(|| key.to_string())
    }

    pub fn creds(&self) -> Creds {
        let cfg = self.cfg();
        Creds {
            client_id: cfg.client_id.clone(),
            client_secret: self.secrets.client_secret(),
            redirect_uri: cfg.redirect_uri.clone(),
            scopes: cfg.scopes.clone(),
        }
    }

    fn push_activity(&self, key: &str, what: &'static str) {
        let label = self.label_of(key);
        let mut ui = self.ui.lock().unwrap();
        ui.activity.push_front(Activity { at: chrono::Local::now().format("%H:%M").to_string(), key: key.into(), label, what });
        ui.activity.truncate(50);
    }

    // -------------------------------------------------------- watching --

    /// v1 Start: validate, then run the watcher until Stop.
    pub fn start(self: &Arc<Self>) -> Result<(), String> {
        {
            let run = self.run.lock().unwrap();
            if run.watching {
                return Ok(());
            }
        }
        let cfg = self.cfg();
        if cfg.client_id.trim().is_empty() || !self.secrets.has_secret() {
            return Err("needs-credentials".into());
        }
        if cfg.targets.is_empty() {
            return Err("Add at least one person to watch first.".into());
        }
        if !cfg.sound_file.is_empty() {
            self.bank.default_clip(&cfg)?;
        }
        let stop = CancellationToken::new();
        {
            let mut run = self.run.lock().unwrap();
            run.watching = true;
            run.stopping = false;
            run.members.clear();
            run.status = Status::Connecting;
            run.detail.clear();
            run.stop = Some(stop.clone());
        }
        let host: Arc<dyn Host> = Arc::new(Bridge(self.clone()));
        let creds = self.creds();
        tauri::async_runtime::spawn(vc_core::discord::run_forever(
            host,
            creds,
            vc_core::discord::http_token_fn(),
            stop,
            ipc_override(),
        ));
        self.changed();
        Ok(())
    }

    pub fn stop(&self) {
        let token = {
            let mut run = self.run.lock().unwrap();
            if !run.watching {
                return;
            }
            run.stopping = true;
            run.stop.take()
        };
        if let Some(t) = token {
            t.cancel();
        }
        self.changed();
    }

    fn finished(&self) {
        let mut run = self.run.lock().unwrap();
        run.watching = false;
        run.stopping = false;
        run.members.clear();
        run.stop = None;
    }
}

/// Tests (and power users) can point the app at another Discord socket/pipe.
pub fn ipc_override() -> Option<PathBuf> {
    std::env::var_os("VC_ALERT_IPC_PATH").map(PathBuf::from)
}

/// Connects the watcher to the app. Settings are read live from the store.
pub struct Bridge(pub Arc<AppState>);

impl Host for Bridge {
    fn targets(&self) -> Vec<String> {
        self.0.store.lock().unwrap().cfg.keys()
    }
    fn cooldown(&self) -> f64 {
        self.0.store.lock().unwrap().cfg.cooldown_seconds
    }
    fn quiet(&self) -> bool {
        self.0.store.lock().unwrap().cfg.general.quiet
    }
    fn alert_if_present(&self) -> bool {
        self.0.store.lock().unwrap().cfg.alert_if_already_present
    }
    fn load_token(&self) -> Option<Token> {
        self.0.secrets.token()
    }
    fn save_token(&self, token: &Token) {
        if let Err(e) = self.0.secrets.set_token(Some(token.clone())) {
            eprintln!("could not save the Discord login: {e}");
        }
    }
    fn status(&self, status: Status, detail: &str) {
        let s = &self.0;
        match status {
            Status::Stopped => s.finished(),
            Status::SetupError => {
                s.finished();
                s.toast("error", detail);
            }
            _ => {}
        }
        {
            let mut run = s.run.lock().unwrap();
            run.status = status;
            run.detail = detail.to_string();
        }
        s.changed();
    }
    fn presence(&self, members: Vec<Member>) {
        let s = &self.0;
        let before: Vec<String> = s.run.lock().unwrap().members.iter().map(|m| m.key.clone()).collect();
        {
            let mut run = s.run.lock().unwrap();
            if !run.watching {
                return;
            }
            run.members = members.clone();
        }
        for m in &members {
            s.avatars.request(&m.key, &UserRef { id: m.id.clone(), avatar: m.avatar.clone(), username: m.username.clone() });
        }
        for k in before.iter().filter(|k| !members.iter().any(|m| &m.key == *k)) {
            s.push_activity(k, "left");
        }
        s.changed();
    }
    fn alert(&self, key: &str) {
        let s = &self.0;
        s.push_activity(key, "joined");
        s.ui.lock().unwrap().flash = Some((key.to_string(), Instant::now()));
        let _ = s.handle.emit("alert", serde_json::json!({ "key": key, "label": s.label_of(key) }));
        let h = s.handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            crate::overlay::sync(&h);
        });
    }
    fn play(&self, key: &str) {
        let s = &self.0;
        let cfg = s.cfg();
        match s.bank.clip_for(&cfg, key) {
            Ok(c) => s.player.play(c),
            Err(e) => eprintln!("sound for {key}: {e}"),
        }
    }
    fn log(&self, msg: &str) {
        if cfg!(debug_assertions) {
            eprintln!("[{}] {msg}", chrono::Local::now().format("%H:%M:%S"));
        }
    }
}

pub fn state(app: &AppHandle) -> Arc<AppState> {
    app.state::<Arc<AppState>>().inner().clone()
}
