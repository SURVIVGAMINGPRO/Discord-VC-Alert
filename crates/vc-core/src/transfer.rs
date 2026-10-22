//! Import and export of `.json` files.
//!
//! Import understands, by content: a VC Alert 1 `config.json`, v1's old
//! `target_user_ids` list, a v2 shared watch list, a v2 backup (or a copy of
//! `settings.json`), and a plain JSON array of usernames/IDs.
//!
//! Imported files are untrusted: size and count are capped, labels cleaned,
//! unknown keys ignored, and sound *paths* from shared files are never
//! followed (only sounds embedded in the file are accepted, after decoding).
//! Credentials are never applied unless the user ticks the box.
//!
//! Export never writes the Discord login token. A shared watch list never
//! contains credentials, file paths or settings, and its `targets` array uses
//! v1's exact shape so VC Alert 1 can read it too.

use crate::config::{normalize_targets, Config, SoundSpec, Target};
use crate::discord::Token;
use crate::sound::{probe, resolve_path};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_PEOPLE: usize = 2000;
pub const MAX_EMBED_BYTES: usize = 1024 * 1024;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    V1Config,
    V1Legacy,
    Watchlist,
    Backup,
    PlainList,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::V1Config => "VC Alert 1 config",
            Kind::V1Legacy => "VC Alert 1 list",
            Kind::Watchlist => "Shared watch list",
            Kind::Backup => "VC Alert backup",
            Kind::PlainList => "List of usernames",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Folder the file came from (for v1 migration: relative sound files, the
    /// saved login and the picture cache).
    pub source_dir: Option<PathBuf>,
    /// Follow local sound paths. Only for migrating your own v1 folder.
    pub allow_local_files: bool,
}

#[derive(Debug, Clone)]
pub struct Parsed {
    pub kind: Kind,
    pub name: Option<String>,
    pub targets: Vec<Target>,
    pub skipped: Vec<(String, String)>,
    /// Settings sections found in the file, as a patch for [`Config::patched`].
    pub settings: Option<Value>,
    pub credentials: Option<Credentials>,
    /// Embedded sounds by file name (already checked to decode).
    pub sounds: BTreeMap<String, Vec<u8>>,
    /// Local sound files to copy (v1 migration only), by the path the file used.
    pub local_sounds: BTreeMap<String, PathBuf>,
    /// Labels of people whose sound file couldn't come along.
    pub missing_sounds: Vec<String>,
    /// v1 saved login next to the config (migration only).
    pub token: Option<Token>,
    pub avatars_dir: Option<PathBuf>,
}

pub fn parse(text: &str, opts: &ParseOptions) -> Result<Parsed, String> {
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err("That file is too big to be a VC Alert list (over 5 MB).".into());
    }
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("This file isn't valid JSON ({e})."))?;
    let not_ours = || "This file isn't a VC Alert list, backup or config.".to_string();
    let (kind, m) = match &v {
        Value::Array(_) => (Kind::PlainList, None),
        Value::Object(m) => {
            let kind = if let Some(Value::Object(meta)) = m.get("vc_alert") {
                match meta.get("kind").and_then(Value::as_str) {
                    Some("watchlist") => Kind::Watchlist,
                    _ => Kind::Backup,
                }
            } else if ["targets", "client_id", "beep", "overlay", "general"].iter().any(|k| m.contains_key(*k)) {
                Kind::V1Config
            } else if m.contains_key("target_user_ids") {
                Kind::V1Legacy
            } else {
                return Err(not_ours());
            };
            (kind, Some(m))
        }
        _ => return Err(not_ours()),
    };

    let items: Vec<Value> = match (&v, m) {
        (Value::Array(a), _) => a.clone(),
        (_, Some(m)) => {
            let mut items = match m.get("targets") {
                Some(Value::Array(a)) => a.clone(),
                Some(_) => return Err(not_ours()),
                None => Vec::new(),
            };
            if let Some(Value::Array(a)) = m.get("target_user_ids") {
                items.extend(a.iter().cloned());
            }
            items
        }
        _ => Vec::new(),
    };
    if items.len() > MAX_PEOPLE {
        return Err(format!("This file lists {} people. VC Alert lists are capped at 2,000.", items.len()));
    }
    let (mut targets, skipped) = normalize_targets(&items);

    let mut out = Parsed {
        kind,
        name: m
            .and_then(|m| m.get("vc_alert"))
            .and_then(|meta| meta.get("name"))
            .and_then(Value::as_str)
            .map(crate::config::clean_label)
            .filter(|s| !s.is_empty()),
        targets: Vec::new(),
        skipped,
        settings: None,
        credentials: None,
        sounds: BTreeMap::new(),
        local_sounds: BTreeMap::new(),
        missing_sounds: Vec::new(),
        token: None,
        avatars_dir: None,
    };

    if let Some(m) = m {
        // Embedded sounds (v2 exports).
        if let Some(Value::Object(sounds)) = m.get("sounds") {
            for (name, data) in sounds {
                let Some(name) = safe_sound_name(name) else { continue };
                let Some(b64) = data.as_str() else { continue };
                let Ok(bytes) = B64.decode(b64.as_bytes()) else { continue };
                if bytes.len() <= MAX_EMBED_BYTES && probe(&bytes).is_ok() {
                    out.sounds.insert(name, bytes);
                }
            }
        }
        // Settings sections.
        if matches!(kind, Kind::V1Config | Kind::Backup) {
            let mut patch = Map::new();
            for k in ["beep", "overlay", "general", "appearance"] {
                if let Some(Value::Object(sec)) = m.get(k) {
                    patch.insert(k.into(), Value::Object(sec.clone()));
                }
            }
            for k in ["alert_if_already_present", "cooldown_seconds", "beep_profiles"] {
                if let Some(val) = m.get(k) {
                    patch.insert(k.into(), val.clone());
                }
            }
            if let Some(Value::String(f)) = m.get("sound_file") {
                let f = f.trim();
                if !f.is_empty() {
                    if let Some(kept) = keep_sound(f, &mut out, opts) {
                        patch.insert("sound_file".into(), json!(kept));
                    } else {
                        out.missing_sounds.push("Universal sound".into());
                    }
                }
            }
            if !patch.is_empty() {
                out.settings = Some(Value::Object(patch));
            }
        }
        // Credentials (applied only if the user opts in).
        let s = |k: &str| m.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
        let (id, secret) = (s("client_id"), s("client_secret"));
        let placeholder = id.starts_with("PASTE_") || secret.starts_with("PASTE_");
        if !id.is_empty() && !secret.is_empty() && !placeholder {
            out.credentials = Some(Credentials { client_id: id, client_secret: secret });
        }
        // v1 migration extras.
        if opts.allow_local_files {
            if let Some(dir) = &opts.source_dir {
                let tf = m.get("token_file").and_then(Value::as_str).unwrap_or(".vc_alert_token.json");
                out.token = read_v1_token(&resolve_path(tf, dir));
                let av = dir.join("avatars");
                if av.join("index.json").exists() {
                    out.avatars_dir = Some(av);
                }
            }
        }
    }

    // Per-person sound files (join and leave): keep only ones that can come along.
    for t in targets.iter_mut() {
        let label = t.label.clone();
        for slot in [&mut t.sound, &mut t.leave] {
            let Some(spec) = slot.as_mut() else { continue };
            if let Some(file) = spec.file.clone() {
                match keep_sound(&file, &mut out, opts) {
                    Some(kept) => spec.file = Some(kept),
                    None => {
                        spec.file = None;
                        if !out.missing_sounds.contains(&label) {
                            out.missing_sounds.push(label.clone());
                        }
                    }
                }
            }
            if spec.is_empty() {
                *slot = None;
            }
        }
    }
    out.targets = targets;
    Ok(out)
}

/// Decide whether a sound path in an imported file can come along. Returns the
/// name/path to keep in the spec until [`apply`] stores it.
fn keep_sound(file: &str, out: &mut Parsed, opts: &ParseOptions) -> Option<String> {
    if let Some(name) = Path::new(file).file_name().and_then(|n| n.to_str()).and_then(safe_sound_name) {
        if out.sounds.contains_key(&name) {
            return Some(name);
        }
    }
    if opts.allow_local_files {
        let base = opts.source_dir.clone().unwrap_or_default();
        let p = resolve_path(file, &base);
        if out.local_sounds.contains_key(file) {
            return Some(file.to_string());
        }
        if let Ok(bytes) = crate::sound::read_capped(&p) {
            if probe(&bytes).is_ok() {
                out.local_sounds.insert(file.to_string(), p);
                return Some(file.to_string());
            }
        }
    }
    None
}

fn safe_sound_name(name: &str) -> Option<String> {
    let base = Path::new(name).file_name()?.to_str()?;
    let clean: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
        .collect();
    let clean = clean.trim().trim_start_matches('.').to_string();
    let ext = Path::new(&clean).extension()?.to_str()?.to_lowercase();
    (["wav", "mp3", "ogg", "flac"].contains(&ext.as_str()) && clean.len() <= 80).then_some(clean)
}

/// v1's `.vc_alert_token.json`.
pub fn read_v1_token(p: &Path) -> Option<Token> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
    let mut scopes: Vec<String> = v.get("scopes")?.as_array()?.iter().filter_map(|s| s.as_str().map(str::to_string)).collect();
    scopes.sort();
    Some(Token {
        client_id: v.get("client_id")?.as_str()?.to_string(),
        scopes,
        access_token: v.get("access_token")?.as_str()?.to_string(),
        refresh_token: v.get("refresh_token").and_then(Value::as_str).map(str::to_string),
    })
}

// --------------------------------------------------------------- preview --

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub key: String,
    pub label: String,
    pub username: Option<String>,
    pub id: Option<String>,
    /// "new", "same" or "different"
    pub status: &'static str,
    pub mine_label: Option<String>,
    pub has_sound: bool,
    pub sound_differs: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skip {
    pub value: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub kind: Kind,
    pub kind_label: &'static str,
    pub name: Option<String>,
    pub total: usize,
    pub new: usize,
    pub same: usize,
    pub different: usize,
    pub rows: Vec<Row>,
    pub skipped: Vec<Skip>,
    pub has_settings: bool,
    pub has_credentials: bool,
    pub has_token: bool,
    pub has_avatars: bool,
    pub missing_sounds: Vec<String>,
    pub embedded_sounds: usize,
}

pub fn preview(p: &Parsed, cur: &Config) -> Preview {
    let rows: Vec<Row> = p
        .targets
        .iter()
        .map(|t| {
            let mine = cur.find(t.key());
            let sound_differs = mine.is_some_and(|m| m.sound != t.sound || m.leave_sound != t.leave_sound);
            let status = match mine {
                None => "new",
                Some(m) if m.label == t.label && !sound_differs => "same",
                Some(_) => "different",
            };
            Row {
                key: t.key().to_string(),
                label: t.label.clone(),
                username: t.username.clone(),
                id: t.id.clone(),
                status,
                mine_label: mine.map(|m| m.label.clone()),
                has_sound: t.sound.is_some(),
                sound_differs,
            }
        })
        .collect();
    let count = |s: &str| rows.iter().filter(|r| r.status == s).count();
    Preview {
        kind: p.kind,
        kind_label: p.kind.label(),
        name: p.name.clone(),
        total: rows.len(),
        new: count("new"),
        same: count("same"),
        different: count("different"),
        skipped: p.skipped.iter().map(|(v, r)| Skip { value: v.clone(), reason: r.clone() }).collect(),
        rows,
        has_settings: p.settings.is_some(),
        has_credentials: p.credentials.is_some(),
        has_token: p.token.is_some(),
        has_avatars: p.avatars_dir.is_some(),
        missing_sounds: p.missing_sounds.clone(),
        embedded_sounds: p.sounds.len(),
    }
}

// ----------------------------------------------------------------- apply --

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyOptions {
    /// "merge" (default) or "replace"
    pub mode: String,
    /// Keys of the people to import.
    pub selected: Vec<String>,
    /// Keys where the file's version wins over yours.
    #[serde(default)]
    pub theirs: Vec<String>,
    #[serde(default)]
    pub import_settings: bool,
    #[serde(default)]
    pub import_credentials: bool,
    /// Import the file's people as a new list with this name (it becomes the
    /// watched list; your others stay as they are).
    #[serde(default)]
    pub new_list: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Applied {
    pub cfg: Config,
    pub credentials: Option<Credentials>,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

/// Merge or replace. `data_dir` is where `sounds/` lives; sound paths are
/// stored relative to it.
pub fn apply(p: &Parsed, cur: &Config, o: &ApplyOptions, data_dir: &Path) -> Result<Applied, String> {
    // As a new list: start from a copy with a fresh, empty list switched to,
    // so the merge below fills it and leaves every other list alone.
    let fresh;
    let cur = match o.new_list.as_deref() {
        Some(name) => {
            let mut c = cur.clone();
            c.add_list(name)?;
            fresh = c;
            &fresh
        }
        None => cur,
    };
    let lower = |v: &[String]| v.iter().map(|k| crate::config::norm_key(k)).collect::<Vec<_>>();
    let selected = lower(&o.selected);
    let theirs = lower(&o.theirs);
    let is_sel = |t: &Target| selected.contains(&t.key().to_lowercase());
    let wins = |t: &Target| theirs.contains(&t.key().to_lowercase());

    let store_sound = |spec: &Option<SoundSpec>| -> Result<Option<SoundSpec>, String> {
        let Some(mut s) = spec.clone() else { return Ok(None) };
        if let Some(f) = s.file.clone() {
            s.file = Some(store_sound_file(p, &f, data_dir)?);
        }
        Ok(Some(s))
    };

    let mut out: Vec<Target>;
    let (mut added, mut updated, mut removed) = (0, 0, 0);
    if o.mode == "replace" {
        out = Vec::new();
        for t in p.targets.iter().filter(|t| is_sel(t)) {
            match cur.find(t.key()) {
                Some(mine) if !wins(t) => out.push(mine.clone()),
                Some(_) => {
                    let mut n = t.clone();
                    n.sound = store_sound(&t.sound)?;
                    n.leave = store_sound(&t.leave)?;
                    out.push(n);
                    updated += 1;
                }
                None => {
                    let mut n = t.clone();
                    n.sound = store_sound(&t.sound)?;
                    n.leave = store_sound(&t.leave)?;
                    out.push(n);
                    added += 1;
                }
            }
        }
        removed = cur.targets.iter().filter(|m| !out.iter().any(|t| t.key() == m.key())).count();
    } else {
        out = cur.targets.clone();
        for t in p.targets.iter().filter(|t| is_sel(t)) {
            if let Some(i) = out.iter().position(|m| m.key().eq_ignore_ascii_case(t.key())) {
                if wins(t) {
                    out[i].label = t.label.clone();
                    out[i].sound = store_sound(&t.sound)?;
                    out[i].leave_sound = t.leave_sound;
                    out[i].leave = store_sound(&t.leave)?;
                    updated += 1;
                }
            } else {
                let mut n = t.clone();
                n.sound = store_sound(&t.sound)?;
                n.leave = store_sound(&t.leave)?;
                out.push(n);
                added += 1;
            }
        }
    }
    if out.len() > MAX_PEOPLE {
        return Err("That would put more than 2,000 people on your list.".into());
    }

    let mut cfg = cur.clone();
    cfg.targets = out;
    if o.import_settings {
        if let Some(patch) = &p.settings {
            let mut patch = patch.clone();
            if let Some(Value::String(f)) = patch.get("sound_file").cloned() {
                patch["sound_file"] = json!(store_sound_file(p, &f, data_dir)?);
            }
            // Beep profiles merge instead of replacing: yours stay first, the
            // file's fill the free slots (same name = yours wins, max 3).
            if let Some(Value::Array(theirs)) = patch.get("beep_profiles").cloned() {
                let mut merged = match json!(cfg.beep_profiles) {
                    Value::Array(a) => a,
                    _ => Vec::new(),
                };
                merged.extend(theirs);
                patch["beep_profiles"] = Value::Array(merged);
            }
            cfg = cfg.patched(&patch)?;
        }
    }
    let credentials = if o.import_credentials { p.credentials.clone() } else { None };
    if let Some(c) = &credentials {
        cfg.client_id = c.client_id.clone();
    }
    // One more pass through the validators keeps the invariants (no dupes).
    let cfg = Config::from_value(&cfg.to_value())?;
    Ok(Applied { cfg, credentials, added, updated, removed })
}

/// Write an embedded or migrated sound into `data_dir/sounds/` and return the
/// relative path to store. Identical files are reused.
fn store_sound_file(p: &Parsed, file: &str, data_dir: &Path) -> Result<String, String> {
    let (name, bytes) = if let Some(b) = p.sounds.get(file) {
        (file.to_string(), b.clone())
    } else if let Some(src) = p.local_sounds.get(file) {
        let name = src.file_name().and_then(|n| n.to_str()).and_then(safe_sound_name).unwrap_or_else(|| "sound.wav".into());
        (name, crate::sound::read_capped(src)?)
    } else {
        return Ok(file.to_string());
    };
    save_sound(data_dir, &name, &bytes)
}

/// Save sound bytes as `sounds/<name>` (adding -2, -3… if a different file has
/// that name). Returns the path relative to `data_dir`.
pub fn save_sound(data_dir: &Path, name: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = data_dir.join("sounds");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = safe_sound_name(name).unwrap_or_else(|| "sound.wav".into());
    let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or("sound").to_string();
    let ext = Path::new(&name).extension().and_then(|s| s.to_str()).unwrap_or("wav").to_string();
    for i in 1..1000 {
        let candidate = if i == 1 { format!("{stem}.{ext}") } else { format!("{stem}-{i}.{ext}") };
        let path = dir.join(&candidate);
        match std::fs::read(&path) {
            Ok(existing) if existing == bytes => return Ok(format!("sounds/{candidate}")),
            Ok(_) => continue,
            Err(_) => {
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                return Ok(format!("sounds/{candidate}"));
            }
        }
    }
    Err("Too many sound files with that name.".into())
}

// ---------------------------------------------------------------- export --

#[derive(Debug, Clone, Deserialize)]
pub struct ExportOptions {
    /// "watchlist", "backup" or "v1"
    pub kind: String,
    /// Only these people (None = everyone).
    #[serde(default)]
    pub keys: Option<Vec<String>>,
    #[serde(default)]
    pub embed_sounds: bool,
    #[serde(default)]
    pub include_credentials: bool,
    #[serde(default)]
    pub name: Option<String>,
    /// Which of your lists to share (None: the one you're watching). A shared
    /// list carries that list's name unless `name` is given.
    #[serde(default)]
    pub list: Option<usize>,
}

pub fn default_filename(o: &ExportOptions, today: &str) -> String {
    match o.kind.as_str() {
        "backup" if o.include_credentials => format!("vc-alert-backup-{today}-private.json"),
        "backup" => format!("vc-alert-backup-{today}.json"),
        "v1" => "vc-alert-v1-config.json".into(),
        _ => match o.name.as_deref().map(slug).filter(|s| !s.is_empty()) {
            Some(s) => format!("vc-alert-{s}-{today}.json"),
            None => format!("vc-alert-list-{today}.json"),
        },
    }
}

/// "Late night!" → "late-night", for file names.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

pub fn export(
    cfg: &Config,
    o: &ExportOptions,
    client_secret: Option<&str>,
    sound_base: &Path,
    today: &str,
    app_version: &str,
) -> Result<String, String> {
    let wanted: Option<Vec<String>> = o.keys.as_ref().map(|k| k.iter().map(|x| crate::config::norm_key(x)).collect());
    let list = o.list.filter(|_| o.kind != "backup").unwrap_or(cfg.active_list);
    let source = cfg.list_targets(list).ok_or("That list doesn't exist any more.")?;
    let list_name = cfg.lists.get(list).map(|l| l.name.clone());
    let mut targets: Vec<Target> = source
        .iter()
        .filter(|t| wanted.as_ref().is_none_or(|w| w.contains(&t.key().to_lowercase())))
        .cloned()
        .collect();
    let mut sounds = Map::new();
    let mut embed = |file: &str| -> Option<String> {
        if !o.embed_sounds || o.kind == "v1" {
            return None;
        }
        let path = resolve_path(file, sound_base);
        let bytes = std::fs::read(&path).ok()?;
        if bytes.len() > MAX_EMBED_BYTES {
            return None;
        }
        let name = safe_sound_name(path.file_name()?.to_str()?)?;
        let mut unique = name.clone();
        let mut i = 2;
        while let Some(existing) = sounds.get(&unique) {
            if existing.as_str() == Some(&B64.encode(&bytes)) {
                return Some(unique);
            }
            let p = Path::new(&name);
            unique = format!(
                "{}-{i}.{}",
                p.file_stem()?.to_str()?,
                p.extension()?.to_str()?
            );
            i += 1;
        }
        sounds.insert(unique.clone(), json!(B64.encode(&bytes)));
        Some(unique)
    };
    for t in targets.iter_mut() {
        for slot in [&mut t.sound, &mut t.leave] {
            if let Some(spec) = slot.as_mut() {
                if let Some(f) = spec.file.clone() {
                    spec.file = embed(&f);
                }
                if spec.is_empty() {
                    *slot = None;
                }
            }
        }
    }

    let header = |kind: &str| {
        let mut h = Map::new();
        h.insert("kind".into(), json!(kind));
        h.insert("version".into(), json!(crate::config::FILE_VERSION));
        h.insert("exported".into(), json!(today));
        h.insert("app".into(), json!(format!("VC Alert {app_version}")));
        let name = o.name.clone().filter(|n| !n.trim().is_empty()).or(if kind == "watchlist" { list_name.clone() } else { None });
        if let Some(n) = name.map(|n| crate::config::clean_label(&n)).filter(|n| !n.is_empty()) {
            h.insert("name".into(), json!(n));
        }
        Value::Object(h)
    };

    let out = match o.kind.as_str() {
        "v1" => {
            let ov = &cfg.overlay;
            let g = &cfg.general;
            json!({
                "client_id": "",
                "client_secret": "",
                "redirect_uri": cfg.redirect_uri,
                "scopes": cfg.scopes,
                "targets": targets,
                "alert_if_already_present": cfg.alert_if_already_present,
                "cooldown_seconds": cfg.cooldown_seconds,
                "sound_file": "",
                "beep": cfg.beep,
                "overlay": {
                    "enabled": ov.enabled, "corner": ov.corner, "offset": ov.offset,
                    "avatar_px": ov.avatar_px, "layout": ov.layout, "max_shown": ov.max_shown,
                    "opacity": ov.opacity, "bubble": ov.bubble, "hide_when_empty": ov.hide_when_empty
                },
                "general": {
                    "start_watching_on_launch": g.start_watching_on_launch,
                    "start_minimized": g.start_minimized,
                    "mask_usernames": g.mask_usernames,
                    "quiet": g.quiet
                },
                "token_file": ".vc_alert_token.json"
            })
        }
        "backup" => {
            let mut v = cfg.to_value();
            let m = v.as_object_mut().unwrap();
            m.insert("vc_alert".into(), header("backup"));
            m.insert("targets".into(), json!(targets));
            m.remove("client_id");
            if o.include_credentials && !cfg.client_id.is_empty() {
                m.insert("client_id".into(), json!(cfg.client_id));
                if let Some(s) = client_secret.filter(|s| !s.is_empty()) {
                    m.insert("client_secret".into(), json!(s));
                }
            }
            let universal = if cfg.sound_file.is_empty() { None } else { embed(&cfg.sound_file) };
            m.insert("sound_file".into(), json!(universal.unwrap_or_default()));
            if !sounds.is_empty() {
                m.insert("sounds".into(), Value::Object(sounds.clone()));
            }
            v
        }
        _ => {
            let mut m = Map::new();
            m.insert("vc_alert".into(), header("watchlist"));
            m.insert("targets".into(), json!(targets));
            if !sounds.is_empty() {
                m.insert("sounds".into(), Value::Object(sounds.clone()));
            }
            Value::Object(m)
        }
    };
    serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
}

/// Copy `settings.json` into `backups/` before a big change; keep the last 10.
pub fn backup(settings: &Path, backups: &Path, stamp: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(backups).map_err(|e| e.to_string())?;
    let dest = backups.join(format!("settings-{stamp}.json"));
    std::fs::copy(settings, &dest).map_err(|e| e.to_string())?;
    let mut all: Vec<PathBuf> = std::fs::read_dir(backups)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    all.sort();
    while all.len() > 10 {
        let _ = std::fs::remove_file(all.remove(0));
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PartialBeep, Target};

    fn cfg_with(names: &[&str]) -> Config {
        let mut c = Config::default();
        for n in names {
            c.targets.push(Target::new("", crate::config::parse_identifier(n).unwrap()));
        }
        c
    }

    #[test]
    fn detects_kinds() {
        let o = ParseOptions::default();
        assert_eq!(parse(r#"["alpha","beta"]"#, &o).unwrap().kind, Kind::PlainList);
        assert_eq!(parse(r#"{"targets":[]}"#, &o).unwrap().kind, Kind::V1Config);
        assert_eq!(parse(r#"{"target_user_ids":["123456789012345678"]}"#, &o).unwrap().kind, Kind::V1Legacy);
        assert_eq!(parse(r#"{"vc_alert":{"kind":"watchlist"},"targets":[]}"#, &o).unwrap().kind, Kind::Watchlist);
        assert_eq!(parse(r#"{"vc_alert":{"version":2},"targets":[]}"#, &o).unwrap().kind, Kind::Backup);
        assert!(parse(r#"{"hello":1}"#, &o).is_err());
        assert!(parse("not json", &o).is_err());
        assert!(parse("42", &o).is_err());
    }

    #[test]
    fn preview_and_merge() {
        let cur = cfg_with(&["alpha", "beta"]);
        let text = r#"{"targets":[{"label":"Alpha!","username":"alpha"},{"label":"beta","username":"beta"},
            {"label":"Gamma","username":"gamma"},"bad name"],"client_id":"1","client_secret":"s"}"#;
        let p = parse(text, &ParseOptions::default()).unwrap();
        let pv = preview(&p, &cur);
        assert_eq!((pv.new, pv.same, pv.different), (1, 1, 1));
        assert_eq!(pv.skipped.len(), 1);
        assert!(pv.has_credentials);
        let dir = tempfile::tempdir().unwrap();
        let o = ApplyOptions {
            mode: "merge".into(),
            selected: vec!["alpha".into(), "beta".into(), "gamma".into()],
            theirs: vec![],
            import_settings: false,
            import_credentials: false,
            new_list: None,
        };
        let a = apply(&p, &cur, &o, dir.path()).unwrap();
        assert_eq!(a.cfg.targets.len(), 3);
        assert_eq!(a.cfg.targets[0].label, "alpha", "mine wins by default");
        assert!(a.credentials.is_none());
        assert_eq!(a.cfg.client_id, "");
        let o2 = ApplyOptions { theirs: vec!["ALPHA".into()], import_credentials: true, ..o };
        let a = apply(&p, &cur, &o2, dir.path()).unwrap();
        assert_eq!(a.cfg.targets[0].label, "Alpha!");
        assert_eq!(a.credentials.unwrap().client_secret, "s");
    }

    #[test]
    fn imported_profiles_merge_with_yours() {
        let mut cur = Config::default();
        cur.beep_profiles = crate::config::clean_profiles(Some(&json!([
            {"name": "Mine", "beep": {"frequency_hz": 500}},
            {"name": "Shared", "beep": {"frequency_hz": 600}}
        ])));
        let text = r#"{"vc_alert": {"version": 2}, "targets": [], "beep_profiles": [
            {"name": "shared", "beep": {"frequency_hz": 900}},
            {"name": "Theirs", "beep": {"frequency_hz": 1200}},
            {"name": "Extra", "beep": {}}
        ]}"#;
        let p = parse(text, &ParseOptions::default()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let o = ApplyOptions { mode: "merge".into(), selected: vec![], theirs: vec![], import_settings: true, import_credentials: false, new_list: None };
        let a = apply(&p, &cur, &o, dir.path()).unwrap();
        let names: Vec<&str> = a.cfg.beep_profiles.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Mine", "Shared", "Theirs"]);
        assert_eq!(a.cfg.beep_profiles[1].beep.frequency_hz, 600, "yours wins on a name clash");
    }

    #[test]
    fn replace_mode() {
        let cur = cfg_with(&["alpha", "beta"]);
        let p = parse(r#"["beta","gamma"]"#, &ParseOptions::default()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let o = ApplyOptions {
            mode: "replace".into(),
            selected: vec!["beta".into(), "gamma".into()],
            theirs: vec![],
            import_settings: false,
            import_credentials: false,
            new_list: None,
        };
        let a = apply(&p, &cur, &o, dir.path()).unwrap();
        assert_eq!(a.cfg.keys(), vec!["beta", "gamma"]);
        assert_eq!(a.removed, 1);
    }

    #[test]
    fn shared_files_cannot_point_at_local_files() {
        let dir = tempfile::tempdir().unwrap();
        let wav = crate::sound::wav_bytes(&crate::sound::beep_samples(&Default::default()));
        std::fs::write(dir.path().join("horn.wav"), &wav).unwrap();
        let text = format!(
            r#"{{"targets":[{{"label":"T","username":"tt","sound":{{"file":"{}"}}}}]}}"#,
            dir.path().join("horn.wav").display().to_string().replace('\\', "\\\\")
        );
        let p = parse(&text, &ParseOptions::default()).unwrap();
        assert!(p.targets[0].sound.is_none());
        assert_eq!(p.missing_sounds, vec!["T"]);
        // ...but migrating your own v1 folder may.
        let o = ParseOptions { source_dir: Some(dir.path().into()), allow_local_files: true };
        let p = parse(r#"{"targets":[{"label":"T","username":"tt","sound":{"file":"horn.wav"}}]}"#, &o).unwrap();
        assert!(p.targets[0].sound.is_some());
        let data = tempfile::tempdir().unwrap();
        let a = apply(&p, &Config::default(), &ApplyOptions {
            mode: "merge".into(), selected: vec!["tt".into()], theirs: vec![], import_settings: false, import_credentials: false, new_list: None,
        }, data.path()).unwrap();
        assert_eq!(a.cfg.targets[0].sound.as_ref().unwrap().file.as_deref(), Some("sounds/horn.wav"));
        assert!(data.path().join("sounds/horn.wav").exists());
    }

    #[test]
    fn export_round_trip_with_embedded_sound_and_no_secrets() {
        let data = tempfile::tempdir().unwrap();
        let wav = crate::sound::wav_bytes(&crate::sound::beep_samples(&Default::default()));
        let rel = save_sound(data.path(), "horn.wav", &wav).unwrap();
        let mut cfg = cfg_with(&["alpha", "maria_k"]);
        cfg.client_id = "123".into();
        cfg.targets[1].label = "Maria".into();
        cfg.targets[1].sound = Some(SoundSpec { file: Some(rel), beep: Some(PartialBeep { repeats: Some(2), ..Default::default() }) });
        for kind in ["watchlist", "backup", "v1"] {
            let o = ExportOptions { kind: kind.into(), keys: None, embed_sounds: true, include_credentials: false, name: Some("Friends".into()), list: None };
            let text = export(&cfg, &o, Some("SECRET"), data.path(), "2026-10-21", "2.0.0").unwrap();
            assert!(!text.contains("SECRET"), "{kind} leaked the secret");
            assert!(!text.contains("access_token"));
            assert!(!text.contains(&data.path().display().to_string()), "{kind} leaked a local path");
            let p = parse(&text, &ParseOptions::default()).unwrap();
            assert_eq!(p.targets.len(), 2, "{kind}");
            if kind != "v1" {
                assert_eq!(p.sounds.len(), 1, "{kind}");
                assert_eq!(p.targets[1].sound.as_ref().unwrap().file.as_deref(), Some("horn.wav"));
            }
            // v1 can read the targets array: same shape.
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["targets"][1]["label"], "Maria");
            assert_eq!(v["targets"][1]["username"], "maria_k");
        }
        let o = ExportOptions { kind: "backup".into(), keys: None, embed_sounds: false, include_credentials: true, name: None, list: None };
        let text = export(&cfg, &o, Some("SECRET"), data.path(), "2026-10-21", "2.0.0").unwrap();
        assert!(text.contains("SECRET"));
        assert!(default_filename(&o, "2026-10-21").ends_with("-private.json"));
        let only = ExportOptions { kind: "watchlist".into(), keys: Some(vec!["@MARIA_K".into()]), embed_sounds: false, include_credentials: false, name: None, list: None };
        let text = export(&cfg, &only, None, data.path(), "d", "2").unwrap();
        let p = parse(&text, &ParseOptions::default()).unwrap();
        assert_eq!(p.targets.len(), 1);
        assert!(p.targets[0].sound.as_ref().unwrap().file.is_none());
    }

    #[test]
    fn hostile_embedded_names_are_cleaned() {
        assert_eq!(safe_sound_name("../../evil.wav").as_deref(), Some("evil.wav"));
        assert_eq!(safe_sound_name("x.exe"), None);
        assert_eq!(safe_sound_name(".hidden.wav").as_deref(), Some("hidden.wav"));
    }

    #[test]
    fn a_shared_list_never_carries_a_leave_sound_path() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = cfg_with(&["alpha"]);
        cfg.targets[0].leave_sound = false;
        cfg.targets[0].leave = Some(SoundSpec { file: Some("/somewhere/private/leave.wav".into()), beep: None });
        let o = ExportOptions { kind: "watchlist".into(), keys: None, embed_sounds: false, include_credentials: false, name: None, list: None };
        let text = export(&cfg, &o, None, dir.path(), "2026-10-22", "test").unwrap();
        assert!(!text.contains("/somewhere"), "no file paths in a shared list");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["targets"][0]["leave_sound"], json!(false), "switching it off for them still travels");
        assert!(v["targets"][0].get("leave").is_none());
        // And an imported path is never trusted.
        let p = parse(r#"{"vc_alert":{"kind":"watchlist"},"targets":[{"label":"A","username":"alpha","leave_sound":true,
            "leave":{"file":"/etc/passwd"}}]}"#, &ParseOptions::default()).unwrap();
        assert!(p.targets[0].leave.is_none());
        assert!(p.targets[0].leave_sound);
    }

    #[test]
    fn one_list_out_and_in_as_a_new_list() {
        let dir = tempfile::tempdir().unwrap();
        // Two lists: "Main" (watched) and "Gaming" with its own sound for maria_k.
        let mut cfg = cfg_with(&["alpha"]);
        let mut gaming = cfg_with(&["maria_k", "beta"]).targets;
        gaming[0].sound = Some(SoundSpec { file: None, beep: Some(PartialBeep { repeats: Some(4), ..Default::default() }) });
        cfg.lists.push(crate::config::WatchList { name: "Gaming".into(), targets: gaming });

        let o = ExportOptions { kind: "watchlist".into(), keys: None, embed_sounds: false, include_credentials: false, name: None, list: Some(1) };
        let text = export(&cfg, &o, None, dir.path(), "2026-10-22", "test").unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["vc_alert"]["name"], json!("Gaming"), "a shared list carries its name");
        assert_eq!(v["targets"].as_array().unwrap().len(), 2, "that list's people, not the watched list's");
        assert_eq!(v["targets"][0]["sound"]["beep"]["repeats"], json!(4), "with that list's sounds");
        assert_eq!(default_filename(&ExportOptions { name: Some("Late night!".into()), ..o.clone() }, "2026-10-22"), "vc-alert-late-night-2026-10-22.json");
        assert!(export(&cfg, &ExportOptions { list: Some(7), ..o.clone() }, None, dir.path(), "d", "t").is_err());

        // Import it on another computer as a new list.
        let p = parse(&text, &ParseOptions::default()).unwrap();
        assert_eq!(p.name.as_deref(), Some("Gaming"));
        let keys: Vec<String> = p.targets.iter().map(|t| t.key().to_string()).collect();
        let a = ApplyOptions { mode: "merge".into(), selected: keys, theirs: vec![], import_settings: false, import_credentials: false, new_list: Some("Gaming".into()) };
        let mine = cfg_with(&["alpha", "maria_k"]);
        let got = apply(&p, &mine, &a, dir.path()).unwrap().cfg;
        assert_eq!(got.lists.len(), 2);
        assert_eq!(got.lists[got.active_list].name, "Gaming", "the new list is the watched one");
        assert_eq!(got.targets.len(), 2);
        assert_eq!(got.lists[0].targets.len(), 2, "the list you had is untouched");
        let mut full = mine.clone();
        for n in ["B", "C", "D", "E"] {
            full.add_list(n).unwrap();
        }
        assert!(apply(&p, &full, &a, dir.path()).unwrap_err().contains("5 lists"));
    }
}

