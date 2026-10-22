//! Settings: the schema, defaults, and the lenient validation rules ported from
//! VC Alert 1 (`vc_alert.py`). Anything read from disk or from an imported file
//! goes through [`Config::from_value`], so bad values fall back to defaults and
//! numbers are clamped exactly like v1 did.
//!
//! The file format is v1's `config.json` minus the secrets (those live in the OS
//! keychain now) plus a few v2 sections. v1 ignores keys it doesn't know, so a
//! v2 file still loads in v1.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const CORNERS: [&str; 4] = ["top-left", "top-right", "bottom-left", "bottom-right"];
pub const LAYOUTS: [&str; 3] = ["row", "column", "grid"];
pub const BUBBLES: [&str; 3] = ["username", "label", "both"];
pub const STYLES: [&str; 3] = ["pictures", "count", "dots"];
pub const THEMES: [&str; 5] = ["onyx", "graphite", "night", "paper", "system"];
pub const DENSITIES: [&str; 2] = ["compact", "comfortable"];

pub const AVATAR_MIN: f64 = 8.0;
pub const AVATAR_MAX: f64 = 48.0;
pub const MAX_SHOWN: f64 = 8.0;
pub const LABEL_MAX: usize = 64;

pub const PARSE_ERROR: &str = "Enter a Discord username (letters, numbers, _ and . only; \
this is the unique username, not the display name) or a numeric user ID.";

// ------------------------------------------------------------ identifiers --

/// What the user typed, understood as a Discord user ID or a unique username.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ident {
    Id(String),
    Username(String),
}

impl Ident {
    pub fn key(&self) -> &str {
        match self {
            Ident::Id(s) | Ident::Username(s) => s,
        }
    }
}

/// v1 `parse_identifier`: 15+ ASCII digits is an ID; otherwise a 2–32 character
/// username of `[a-z0-9_.]` (case-insensitive, leading `@` ignored).
pub fn parse_identifier(text: &str) -> Result<Ident, String> {
    let raw = text.trim().trim_start_matches('@').trim();
    if raw.len() >= 15 && raw.chars().all(|c| c.is_ascii_digit()) {
        return Ok(Ident::Id(raw.to_string()));
    }
    let name = raw.to_lowercase();
    let n = name.chars().count();
    if (2..=32).contains(&n)
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Ok(Ident::Username(name));
    }
    Err(PARSE_ERROR.to_string())
}

/// Normalise a lookup key the way v1's `TargetStore.get` did.
pub fn norm_key(key: &str) -> String {
    key.trim().trim_start_matches('@').to_lowercase()
}

/// Labels come from files other people made: drop control characters and cap
/// the length so a hostile file can't wreck the layout.
pub fn clean_label(raw: &str) -> String {
    let s: String = raw.chars().filter(|c| !c.is_control()).collect();
    let s = s.trim();
    s.chars().take(LABEL_MAX).collect::<String>().trim().to_string()
}

// --------------------------------------------------------------- numbers --

/// v1 `_num`: coerce to a number clamped to [lo, hi], else `default`.
/// Booleans and NaN are not numbers. Integers use Python's round-half-even.
pub fn num(v: Option<&Value>, lo: f64, hi: f64, default: f64, as_int: bool) -> f64 {
    let x = match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(default),
        _ => default,
    };
    let x = if x.is_nan() { default } else { x };
    let x = x.clamp(lo, hi);
    if as_int {
        x.round_ties_even()
    } else {
        x
    }
}

fn is_number(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Number(_)))
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round_ties_even() / 100.0
}

// ------------------------------------------------------------------ beep --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Beep {
    pub frequency_hz: i64,
    pub duration_ms: i64,
    pub repeats: i64,
    pub gap_ratio: f64,
    pub volume: f64,
}

impl Default for Beep {
    fn default() -> Self {
        Beep { frequency_hz: 1000, duration_ms: 180, repeats: 3, gap_ratio: 0.5, volume: 0.6 }
    }
}

/// A per-person beep: only the keys it overrides.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialBeep {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub frequency_hz: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub repeats: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gap_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub volume: Option<f64>,
}

impl PartialBeep {
    pub fn is_empty(&self) -> bool {
        self.frequency_hz.is_none()
            && self.duration_ms.is_none()
            && self.repeats.is_none()
            && self.gap_ratio.is_none()
            && self.volume.is_none()
    }

    pub fn apply_to(&self, base: &Beep) -> Beep {
        Beep {
            frequency_hz: self.frequency_hz.unwrap_or(base.frequency_hz),
            duration_ms: self.duration_ms.unwrap_or(base.duration_ms),
            repeats: self.repeats.unwrap_or(base.repeats),
            gap_ratio: self.gap_ratio.unwrap_or(base.gap_ratio),
            volume: self.volume.unwrap_or(base.volume),
        }
    }
}

/// v1 `clean_beep` without a base: keep only the numeric keys that are present.
pub fn clean_partial_beep(raw: Option<&Value>) -> PartialBeep {
    let Some(Value::Object(m)) = raw else { return PartialBeep::default() };
    let int = |k: &str, lo, hi, d| {
        is_number(m.get(k)).then(|| num(m.get(k), lo, hi, d, true) as i64)
    };
    let flt = |k: &str, lo, hi, d| is_number(m.get(k)).then(|| num(m.get(k), lo, hi, d, false));
    PartialBeep {
        frequency_hz: int("frequency_hz", 100.0, 5000.0, 1000.0),
        duration_ms: int("duration_ms", 20.0, 2000.0, 180.0),
        repeats: int("repeats", 1.0, 10.0, 3.0),
        volume: flt("volume", 0.0, 1.0, 0.6),
        gap_ratio: flt("gap_ratio", 0.0, 3.0, 0.5),
    }
}

/// The universal beep: the file's keys merged over the defaults, with v1's
/// legacy `gap_ms` converted to a ratio of the beep length.
pub fn clean_beep(raw: Option<&Value>) -> Beep {
    let mut merged = match serde_json::to_value(Beep::default()) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    };
    if let Some(Value::Object(m)) = raw {
        for (k, v) in m {
            merged.insert(k.clone(), v.clone());
        }
        if m.contains_key("gap_ms") && !m.contains_key("gap_ratio") {
            let gap = m.get("gap_ms").and_then(as_f64_loose);
            let dur = merged.get("duration_ms").and_then(as_f64_loose);
            if let (Some(g), Some(d)) = (gap, dur) {
                if d != 0.0 {
                    merged.insert("gap_ratio".into(), json!(g / d));
                }
            }
        }
        merged.remove("gap_ms");
    }
    let p = clean_partial_beep(Some(&Value::Object(merged)));
    p.apply_to(&Beep::default())
}

fn as_f64_loose(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

// ----------------------------------------------------------------- sound --

/// A person's own sound: a file and/or a partial beep (v1 shape).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SoundSpec {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub beep: Option<PartialBeep>,
}

impl SoundSpec {
    pub fn is_empty(&self) -> bool {
        self.file.is_none() && self.beep.as_ref().is_none_or(|b| b.is_empty())
    }
}

/// v1 `clean_sound`: None when it says nothing.
pub fn clean_sound(raw: Option<&Value>) -> Option<SoundSpec> {
    let Some(Value::Object(m)) = raw else { return None };
    let file = match m.get("file") {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    };
    let beep = clean_partial_beep(m.get("beep"));
    let spec = SoundSpec { file, beep: (!beep.is_empty()).then_some(beep) };
    (!spec.is_empty()).then_some(spec)
}

// --------------------------------------------------------------- targets --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sound: Option<SoundSpec>,
}

impl Target {
    pub fn new(label: &str, ident: Ident) -> Target {
        let label = clean_label(label);
        let label = if label.is_empty() { ident.key().to_string() } else { label };
        let (id, username) = match ident {
            Ident::Id(s) => (Some(s), None),
            Ident::Username(s) => (None, Some(s)),
        };
        Target { label, id, username, sound: None }
    }

    /// v1 `target_key`: the numeric ID, else the username.
    pub fn key(&self) -> &str {
        self.id.as_deref().or(self.username.as_deref()).unwrap_or("")
    }
}

/// v1 `normalize_targets`: accepts `{label, id|username, sound?}` objects or bare
/// strings/numbers (legacy IDs); drops invalid entries and duplicates; keeps order.
pub fn normalize_targets(items: &[Value]) -> (Vec<Target>, Vec<(String, String)>) {
    let mut out: Vec<Target> = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in items {
        let (label, source, sound) = match item {
            Value::Object(m) => {
                let label = match m.get("label") {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Null) | None => String::new(),
                    Some(other) => other.to_string(),
                };
                let source = [m.get("id"), m.get("username")]
                    .into_iter()
                    .flatten()
                    .find(|v| truthy(v))
                    .map(value_to_text)
                    .unwrap_or_default();
                (label, source, clean_sound(m.get("sound")))
            }
            other => (String::new(), value_to_text(other), None),
        };
        let shown = if source.is_empty() { label.clone() } else { source.clone() };
        let ident = match parse_identifier(&source) {
            Ok(i) => i,
            Err(_) => {
                skipped.push((shown, "not a valid username or user ID".to_string()));
                continue;
            }
        };
        if !seen.insert(ident.key().to_string()) {
            skipped.push((shown, "listed twice".to_string()));
            continue;
        }
        let mut t = Target::new(&label, ident);
        t.sound = sound;
        out.push(t);
    }
    (out, skipped)
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

// --------------------------------------------------------------- overlay --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    pub enabled: bool,
    pub corner: String,
    pub offset: [i64; 2],
    pub avatar_px: i64,
    pub layout: String,
    pub max_shown: i64,
    pub opacity: f64,
    pub bubble: String,
    pub hide_when_empty: bool,
    // v2
    pub style: String,
    pub display: String,
    pub hide_from_capture: bool,
}

impl Default for Overlay {
    fn default() -> Self {
        Overlay {
            enabled: true,
            corner: "top-right".into(),
            offset: [24, 24],
            avatar_px: 28,
            layout: "row".into(),
            max_shown: 8,
            opacity: 1.0,
            bubble: "username".into(),
            hide_when_empty: true,
            style: "pictures".into(),
            display: "primary".into(),
            hide_from_capture: true,
        }
    }
}

fn pick(raw: &Map<String, Value>, key: &str, allowed: &[&str], default: &str) -> String {
    match raw.get(key) {
        Some(Value::String(s)) if allowed.contains(&s.as_str()) => s.clone(),
        _ => default.to_string(),
    }
}

fn boolean(raw: &Map<String, Value>, key: &str, default: bool) -> bool {
    match raw.get(key) {
        Some(Value::Bool(b)) => *b,
        _ => default,
    }
}

/// v1 `clean_overlay` plus the v2 keys.
pub fn clean_overlay(raw: Option<&Value>) -> Overlay {
    let empty = Map::new();
    let raw = match raw {
        Some(Value::Object(m)) => m,
        _ => &empty,
    };
    let d = Overlay::default();
    let offset = match raw.get("offset") {
        Some(Value::Array(a)) if a.len() == 2 => [
            num(a.first(), 0.0, 10000.0, 24.0, true) as i64,
            num(a.get(1), 0.0, 10000.0, 24.0, true) as i64,
        ],
        _ => d.offset,
    };
    let display = match raw.get("display") {
        Some(Value::String(s)) if !s.trim().is_empty() => clean_label(s),
        _ => d.display.clone(),
    };
    Overlay {
        enabled: boolean(raw, "enabled", d.enabled),
        corner: pick(raw, "corner", &CORNERS, &d.corner),
        offset,
        avatar_px: num(raw.get("avatar_px").or(Some(&json!(28))), AVATAR_MIN, AVATAR_MAX, 28.0, true)
            as i64,
        layout: pick(raw, "layout", &LAYOUTS, &d.layout),
        max_shown: num(raw.get("max_shown").or(Some(&json!(8))), 1.0, MAX_SHOWN, MAX_SHOWN, true)
            as i64,
        opacity: round2(num(raw.get("opacity").or(Some(&json!(1.0))), 0.3, 1.0, 1.0, false)),
        bubble: pick(raw, "bubble", &BUBBLES, &d.bubble),
        hide_when_empty: boolean(raw, "hide_when_empty", d.hide_when_empty),
        style: pick(raw, "style", &STYLES, &d.style),
        display,
        hide_from_capture: boolean(raw, "hide_from_capture", d.hide_from_capture),
    }
}

// --------------------------------------------------------------- general --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct General {
    pub start_watching_on_launch: bool,
    pub start_minimized: bool,
    pub mask_usernames: bool,
    pub quiet: bool,
    // v2
    pub close_to_tray: bool,
    pub launch_at_login: bool,
    pub hide_dock_icon: bool,
    pub setup_complete: bool,
}

impl Default for General {
    fn default() -> Self {
        General {
            start_watching_on_launch: false,
            start_minimized: false,
            mask_usernames: false,
            quiet: false,
            close_to_tray: true,
            launch_at_login: false,
            hide_dock_icon: false,
            setup_complete: false,
        }
    }
}

pub fn clean_general(raw: Option<&Value>) -> General {
    let empty = Map::new();
    let raw = match raw {
        Some(Value::Object(m)) => m,
        _ => &empty,
    };
    let d = General::default();
    General {
        start_watching_on_launch: boolean(raw, "start_watching_on_launch", d.start_watching_on_launch),
        start_minimized: boolean(raw, "start_minimized", d.start_minimized),
        mask_usernames: boolean(raw, "mask_usernames", d.mask_usernames),
        quiet: boolean(raw, "quiet", d.quiet),
        close_to_tray: boolean(raw, "close_to_tray", d.close_to_tray),
        launch_at_login: boolean(raw, "launch_at_login", d.launch_at_login),
        hide_dock_icon: boolean(raw, "hide_dock_icon", d.hide_dock_icon),
        setup_complete: boolean(raw, "setup_complete", d.setup_complete),
    }
}

// ------------------------------------------------------------ appearance --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub theme: String,
    pub density: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { theme: "onyx".into(), density: "compact".into() }
    }
}

pub fn clean_appearance(raw: Option<&Value>) -> Appearance {
    let empty = Map::new();
    let raw = match raw {
        Some(Value::Object(m)) => m,
        _ => &empty,
    };
    let d = Appearance::default();
    Appearance {
        theme: pick(raw, "theme", &THEMES, &d.theme),
        density: pick(raw, "density", &DENSITIES, &d.density),
    }
}

// ---------------------------------------------------------------- config --

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub targets: Vec<Target>,
    pub alert_if_already_present: bool,
    pub cooldown_seconds: f64,
    pub sound_file: String,
    pub beep: Beep,
    pub overlay: Overlay,
    pub general: General,
    pub appearance: Appearance,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            client_id: String::new(),
            redirect_uri: "http://localhost".into(),
            scopes: vec!["rpc".into(), "rpc.voice.read".into()],
            targets: Vec::new(),
            alert_if_already_present: true,
            cooldown_seconds: 5.0,
            sound_file: String::new(),
            beep: Beep::default(),
            overlay: Overlay::default(),
            general: General::default(),
            appearance: Appearance::default(),
        }
    }
}

pub const FILE_VERSION: i64 = 2;

impl Config {
    /// Build a config from any JSON object (v1 config.json, v2 settings.json,
    /// or a patch merged over one), validating every value like v1 did.
    pub fn from_value(v: &Value) -> Result<Config, String> {
        let Value::Object(m) = v else {
            return Err("must contain a JSON object.".into());
        };
        let d = Config::default();
        let text = |k: &str, default: &str| match m.get(k) {
            Some(Value::String(s)) => s.trim().to_string(),
            _ => default.to_string(),
        };
        let scopes = match m.get("scopes") {
            Some(Value::Array(a)) if !a.is_empty() && a.iter().all(|x| x.is_string()) => {
                a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
            }
            _ => d.scopes.clone(),
        };
        let mut items: Vec<Value> = match m.get("targets") {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        if let Some(Value::Array(a)) = m.get("target_user_ids") {
            items.extend(a.iter().cloned());
        }
        let (targets, _) = normalize_targets(&items);
        let alert = match m.get("alert_if_already_present") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64() != Some(0.0),
            _ => d.alert_if_already_present,
        };
        Ok(Config {
            client_id: text("client_id", ""),
            redirect_uri: {
                let r = text("redirect_uri", &d.redirect_uri);
                if r.is_empty() { d.redirect_uri.clone() } else { r }
            },
            scopes,
            targets,
            alert_if_already_present: alert,
            cooldown_seconds: num(m.get("cooldown_seconds").or(Some(&json!(5))), 0.0, 600.0, 5.0, false),
            sound_file: text("sound_file", ""),
            beep: clean_beep(m.get("beep")),
            overlay: clean_overlay(m.get("overlay")),
            general: clean_general(m.get("general")),
            appearance: clean_appearance(m.get("appearance")),
        })
    }

    /// The on-disk form: v1's keys in v1's order, then the v2 additions.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("vc_alert".into(), json!({ "version": FILE_VERSION }));
        m.insert("client_id".into(), json!(self.client_id));
        m.insert("redirect_uri".into(), json!(self.redirect_uri));
        m.insert("scopes".into(), json!(self.scopes));
        m.insert("targets".into(), json!(self.targets));
        m.insert("alert_if_already_present".into(), json!(self.alert_if_already_present));
        m.insert("cooldown_seconds".into(), json!(self.cooldown_seconds));
        m.insert("sound_file".into(), json!(self.sound_file));
        m.insert("beep".into(), json!(self.beep));
        m.insert("overlay".into(), json!(self.overlay));
        m.insert("general".into(), json!(self.general));
        m.insert("appearance".into(), json!(self.appearance));
        Value::Object(m)
    }

    /// Merge a settings patch (e.g. `{"overlay": {"avatar_px": 20}}`) and
    /// re-validate. Only settings may be patched, never the list or credentials.
    pub fn patched(&self, patch: &Value) -> Result<Config, String> {
        const SECTIONS: [&str; 5] = ["beep", "overlay", "general", "appearance", "scopes"];
        const VALUES: [&str; 4] =
            ["alert_if_already_present", "cooldown_seconds", "sound_file", "redirect_uri"];
        let Value::Object(p) = patch else { return Err("settings patch must be an object".into()) };
        let mut base = self.to_value();
        let Value::Object(b) = &mut base else { unreachable!() };
        for (k, v) in p {
            if SECTIONS.contains(&k.as_str()) {
                match (b.get_mut(k), v) {
                    (Some(Value::Object(cur)), Value::Object(new)) => {
                        for (kk, vv) in new {
                            cur.insert(kk.clone(), vv.clone());
                        }
                    }
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            } else if VALUES.contains(&k.as_str()) {
                b.insert(k.clone(), v.clone());
            } else {
                return Err(format!("unknown setting: {k}"));
            }
        }
        Config::from_value(&base)
    }

    pub fn find(&self, key: &str) -> Option<&Target> {
        let k = norm_key(key);
        self.targets.iter().find(|t| t.key().to_lowercase() == k)
    }

    pub fn position(&self, key: &str) -> Option<usize> {
        let k = norm_key(key);
        self.targets.iter().position(|t| t.key().to_lowercase() == k)
    }

    pub fn keys(&self) -> Vec<String> {
        self.targets.iter().map(|t| t.key().to_string()).collect()
    }
}

/// v1 username masking: keep the first 2 and last 2 characters.
pub fn mask(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let n = chars.len();
    if n <= 4 {
        let first: String = chars.iter().take(1).collect();
        return first + &"•".repeat(n.saturating_sub(1));
    }
    let head: String = chars[..2].iter().collect();
    let tail: String = chars[n - 2..].iter().collect();
    format!("{head}{}{tail}", "•".repeat(n - 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rules() {
        assert_eq!(parse_identifier("@Moderator").unwrap(), Ident::Username("moderator".into()));
        assert_eq!(
            parse_identifier(" 123456789012345678 ").unwrap(),
            Ident::Id("123456789012345678".into())
        );
        assert_eq!(parse_identifier("12345").unwrap(), Ident::Username("12345".into()));
        assert!(parse_identifier("a").is_err());
        assert!(parse_identifier("bad name").is_err());
        assert!(parse_identifier(&"x".repeat(33)).is_err());
        assert_eq!(parse_identifier("abc_._.").unwrap().key(), "abc_._.");
        assert_eq!(parse_identifier("@@@ .dotted ").unwrap().key(), ".dotted");
    }

    #[test]
    fn clamps_match_v1() {
        let o = clean_overlay(Some(&json!({"avatar_px": 500, "max_shown": 0, "opacity": 0.01,
            "corner": "middle", "layout": "spiral", "bubble": "x", "offset": [1, 2, 3], "enabled": 1})));
        assert_eq!(o.avatar_px, 48);
        assert_eq!(o.max_shown, 1);
        assert_eq!(o.opacity, 0.3);
        assert_eq!(o.corner, "top-right");
        assert_eq!(o.layout, "row");
        assert_eq!(o.bubble, "username");
        assert_eq!(o.offset, [24, 24]);
        assert!(o.enabled);
        assert_eq!(clean_overlay(Some(&json!({"avatar_px": 3}))).avatar_px, 8);
        // Python rounds half to even.
        assert_eq!(clean_overlay(Some(&json!({"avatar_px": 28.5}))).avatar_px, 28);
        assert_eq!(clean_overlay(Some(&json!({"offset": ["12", 30.5]}))).offset, [12, 30]);
    }

    #[test]
    fn beep_merge_and_legacy_gap() {
        let b = clean_beep(Some(&json!({"frequency_hz": 9999, "duration_ms": 200, "gap_ms": 100})));
        assert_eq!(b.frequency_hz, 5000);
        assert_eq!(b.duration_ms, 200);
        assert_eq!(b.gap_ratio, 0.5);
        let b = clean_beep(Some(&json!({"volume": true, "repeats": "4"})));
        assert_eq!(b.volume, 0.6);
        assert_eq!(b.repeats, 3, "beep keys must be real numbers, like v1");
    }

    #[test]
    fn targets_normalise() {
        let (t, skipped) = normalize_targets(&[
            json!({"label": " Mod ", "username": "@Moderator"}),
            json!({"label": "dupe", "username": "moderator"}),
            json!(123456789012345678u64),
            json!("bad name"),
            json!({"label": "Maria", "username": "maria_k", "sound": {"beep": {"repeats": 2}, "file": " "}}),
        ]);
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].label, "Mod");
        assert_eq!(t[0].key(), "moderator");
        assert_eq!(t[1].key(), "123456789012345678");
        assert_eq!(t[1].label, "123456789012345678");
        assert_eq!(t[2].sound.as_ref().unwrap().beep.as_ref().unwrap().repeats, Some(2));
        assert!(t[2].sound.as_ref().unwrap().file.is_none());
        assert_eq!(skipped.len(), 2);
    }

    #[test]
    fn legacy_target_ids_migrate() {
        let c = Config::from_value(&json!({"target_user_ids": ["123456789012345678"]})).unwrap();
        assert_eq!(c.targets.len(), 1);
    }

    #[test]
    fn round_trip() {
        let c = Config::from_value(&json!({"targets": [{"label": "Moderator", "username": "moderator"}],
            "overlay": {"corner": "bottom-left", "style": "count"}, "appearance": {"theme": "paper"}}))
        .unwrap();
        let again = Config::from_value(&c.to_value()).unwrap();
        assert_eq!(c, again);
        assert_eq!(again.overlay.style, "count");
    }

    #[test]
    fn patch_only_settings() {
        let c = Config::default();
        let p = c.patched(&json!({"overlay": {"avatar_px": 20}, "cooldown_seconds": 900})).unwrap();
        assert_eq!(p.overlay.avatar_px, 20);
        assert_eq!(p.overlay.corner, "top-right");
        assert_eq!(p.cooldown_seconds, 600.0);
        assert!(c.patched(&json!({"targets": []})).is_err());
    }

    #[test]
    fn masking() {
        assert_eq!(mask("moderator"), "mo•••••or");
        assert_eq!(mask("abcd"), "a•••");
        assert_eq!(mask("ab"), "a•");
    }

    #[test]
    fn labels_cleaned() {
        assert_eq!(clean_label("  a\u{0007}b  "), "ab");
        assert_eq!(clean_label(&"x".repeat(100)).chars().count(), LABEL_MAX);
    }
}
