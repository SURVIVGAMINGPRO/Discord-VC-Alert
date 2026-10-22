//! Importing a real VC Alert 1 folder. Uses `vc_alert_OLD/` next to the
//! project when it exists (it's git-ignored because it holds secrets), and a
//! synthetic v1 config otherwise.

use std::path::PathBuf;
use vc_core::config::Config;
use vc_core::transfer::{apply, parse, preview, ApplyOptions, Kind, ParseOptions};

fn old_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vc_alert_OLD")
}

#[test]
fn imports_the_real_v1_folder_when_present() {
    let dir = old_dir();
    let Ok(text) = std::fs::read_to_string(dir.join("config.json")) else {
        eprintln!("no vc_alert_OLD/config.json here; skipping");
        return;
    };
    let opts = ParseOptions { source_dir: Some(dir.clone()), allow_local_files: true };
    let p = parse(&text, &opts).unwrap();
    assert_eq!(p.kind, Kind::V1Config);
    let pv = preview(&p, &Config::default());
    assert_eq!(pv.total, p.targets.len());
    assert!(pv.total > 0);
    assert!(pv.skipped.is_empty(), "{:?}", pv.skipped);
    assert!(pv.has_settings);
    let data = tempfile::tempdir().unwrap();
    let keys: Vec<String> = p.targets.iter().map(|t| t.key().to_string()).collect();
    let a = apply(&p, &Config::default(), &ApplyOptions {
        mode: "merge".into(), selected: keys, theirs: vec![], import_settings: true, import_credentials: true,
    }, data.path()).unwrap();
    assert_eq!(a.cfg.targets.len(), pv.total);
    // Per-person beeps survive; gap_ratio is never per-person.
    for t in &a.cfg.targets {
        if let Some(b) = t.sound.as_ref().and_then(|s| s.beep.as_ref()) {
            assert!(b.gap_ratio.is_none());
        }
    }
    let raw: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(a.cfg.overlay.avatar_px, raw["overlay"]["avatar_px"].as_i64().unwrap_or(28).clamp(8, 48));
    assert_eq!(a.cfg.overlay.corner, raw["overlay"]["corner"].as_str().unwrap_or("top-right"));
    assert_eq!(a.credentials.is_some(), pv.has_credentials);
}

#[test]
fn imports_a_synthetic_v1_config() {
    let text = r#"{
      "client_id": "PASTE_APPLICATION_ID_HERE", "client_secret": "PASTE_CLIENT_SECRET_HERE",
      "redirect_uri": "http://localhost", "scopes": ["rpc", "rpc.voice.read"],
      "targets": [
        {"label": "Moderator", "username": "moderator"},
        {"label": "Robin", "username": "pine_apple", "sound": {"beep": {"frequency_hz": 1700, "duration_ms": 70, "repeats": 2, "volume": 1.0}}},
        {"label": ".dots", "username": ".dots"},
        {"label": "The Twins", "id": "123456789012345678", "sound": {"file": "sounds/horn.wav"}}
      ],
      "alert_if_already_present": true, "cooldown_seconds": 5.0, "sound_file": "",
      "beep": {"frequency_hz": 1700, "duration_ms": 90, "repeats": 3, "gap_ratio": 0.5, "volume": 1.0},
      "overlay": {"enabled": true, "corner": "top-left", "offset": [24, 24], "avatar_px": 16, "layout": "row",
                  "max_shown": 3, "opacity": 0.6, "bubble": "both", "hide_when_empty": true},
      "general": {"start_watching_on_launch": true, "start_minimized": false, "mask_usernames": false, "quiet": false},
      "token_file": ".vc_alert_token.json"
    }"#;
    let p = parse(text, &ParseOptions::default()).unwrap();
    assert!(p.credentials.is_none(), "placeholder credentials are ignored");
    assert_eq!(p.missing_sounds, vec!["The Twins"]);
    let data = tempfile::tempdir().unwrap();
    let a = apply(&p, &Config::default(), &ApplyOptions {
        mode: "merge".into(),
        selected: p.targets.iter().map(|t| t.key().to_string()).collect(),
        theirs: vec![], import_settings: true, import_credentials: true,
    }, data.path()).unwrap();
    assert_eq!(a.cfg.targets.len(), 4);
    assert_eq!(a.cfg.overlay.avatar_px, 16);
    assert_eq!(a.cfg.overlay.opacity, 0.6);
    assert_eq!(a.cfg.beep.frequency_hz, 1700);
    assert!(a.cfg.general.start_watching_on_launch);
    assert_eq!(a.cfg.overlay.style, "pictures", "v2 keys keep their defaults");
    assert_eq!(a.cfg.targets[1].sound.as_ref().unwrap().beep.as_ref().unwrap().repeats, Some(2));
}
