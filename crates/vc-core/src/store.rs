//! The editable settings backed by `settings.json`. Every write saves at once,
//! atomically (temp file + rename, with v1's retry for Windows file locks).

use crate::config::{clean_label, parse_identifier, Config, SoundSpec, Target};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Store {
    path: PathBuf,
    pub cfg: Config,
}

/// Why a settings file couldn't be loaded.
#[derive(Debug)]
pub enum LoadError {
    /// The file exists but isn't usable JSON; the caller decides what to do.
    Invalid(String),
}

impl Store {
    /// Open (or start fresh at) `path`. An unreadable file is an error so the
    /// app can move it aside instead of silently overwriting it.
    pub fn open(path: &Path) -> Result<Store, LoadError> {
        let cfg = if path.exists() {
            let text = std::fs::read_to_string(path)
                .map_err(|e| LoadError::Invalid(format!("{} could not be read: {e}", path.display())))?;
            let v: Value = serde_json::from_str(&text).map_err(|e| {
                LoadError::Invalid(format!("{} is not valid JSON: {e}", path.display()))
            })?;
            Config::from_value(&v)
                .map_err(|e| LoadError::Invalid(format!("{} {e}", path.display())))?
        } else {
            Config::default()
        };
        Ok(Store { path: path.to_path_buf(), cfg })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn save(&self) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&self.cfg.to_value()).map_err(|e| e.to_string())?;
        write_atomic(&self.path, text.as_bytes())
    }

    /// Replace the whole config (import, restore) and save.
    pub fn replace(&mut self, cfg: Config) -> Result<(), String> {
        self.cfg = cfg;
        self.save()
    }

    pub fn add(&mut self, label: &str, ident: &str) -> Result<Target, String> {
        let parsed = parse_identifier(ident)?;
        if self.cfg.find(parsed.key()).is_some() {
            return Err("That person is already on the list.".into());
        }
        let t = Target::new(label, parsed);
        self.cfg.targets.push(t.clone());
        self.save()?;
        Ok(t)
    }

    /// Add several people at once (a pasted list). Returns the added entries and
    /// the inputs that were skipped, with why.
    pub fn add_many(&mut self, idents: &[String]) -> Result<(Vec<Target>, Vec<(String, String)>), String> {
        let mut added = Vec::new();
        let mut skipped = Vec::new();
        for raw in idents {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            match parse_identifier(raw) {
                Err(_) => skipped.push((raw.to_string(), "not a valid username or user ID".into())),
                Ok(p) if self.cfg.find(p.key()).is_some() => {
                    skipped.push((raw.to_string(), "already on your list".into()))
                }
                Ok(p) => {
                    let t = Target::new("", p);
                    self.cfg.targets.push(t.clone());
                    added.push(t);
                }
            }
        }
        if !added.is_empty() {
            self.save()?;
        }
        Ok((added, skipped))
    }

    /// Put a removed entry back (Undo), keeping its sound and position.
    pub fn insert(&mut self, t: Target, index: usize) -> Result<(), String> {
        if self.cfg.find(t.key()).is_some() || parse_identifier(t.key()).is_err() {
            return Ok(());
        }
        let i = index.min(self.cfg.targets.len());
        self.cfg.targets.insert(i, t);
        self.save()
    }

    pub fn remove(&mut self, key: &str) -> Result<Option<(Target, usize)>, String> {
        let Some(i) = self.cfg.position(key) else { return Ok(None) };
        let t = self.cfg.targets.remove(i);
        self.save()?;
        Ok(Some((t, i)))
    }

    pub fn rename(&mut self, key: &str, label: &str) -> Result<(), String> {
        let Some(i) = self.cfg.position(key) else { return Ok(()) };
        let t = &mut self.cfg.targets[i];
        let l = clean_label(label);
        t.label = if l.is_empty() { t.key().to_string() } else { l };
        self.save()
    }

    pub fn set_sound(&mut self, key: &str, spec: Option<SoundSpec>) -> Result<(), String> {
        let Some(i) = self.cfg.position(key) else { return Ok(()) };
        let spec = spec.and_then(|s| {
            crate::config::clean_sound(Some(&serde_json::to_value(s).unwrap_or(Value::Null)))
        });
        self.cfg.targets[i].sound = spec;
        self.save()
    }

    pub fn patch(&mut self, patch: &Value) -> Result<(), String> {
        let next = self.cfg.patched(patch)?;
        self.cfg = next;
        self.save()
    }

    pub fn set_client_id(&mut self, id: &str) -> Result<(), String> {
        self.cfg.client_id = id.trim().to_string();
        self.save()
    }
}

/// Write via `name.tmp` then rename over the target. Windows can refuse the
/// rename while another program briefly holds the file, so retry like v1.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().ok();
    }
    let mut attempt = 0;
    loop {
        match std::fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 4 => {
                attempt += 1;
                let _ = e;
                std::thread::sleep(std::time::Duration::from_millis(50 * attempt));
            }
            Err(e) => return Err(format!("Could not save {}: {e}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("settings.json")).unwrap();
        (dir, s)
    }

    #[test]
    fn add_remove_undo_keeps_sound_and_index() {
        let (_d, mut s) = store();
        s.add("", "alpha").unwrap();
        s.add("Maria", "@Maria_K").unwrap();
        s.add("", "gamma").unwrap();
        s.set_sound("maria_k", Some(SoundSpec { file: None, beep: Some(crate::config::PartialBeep { repeats: Some(2), ..Default::default() }) })).unwrap();
        assert_eq!(s.add("x", "MARIA_K").unwrap_err(), "That person is already on the list.");
        let (t, i) = s.remove("@maria_k").unwrap().unwrap();
        assert_eq!(i, 1);
        s.insert(t, i).unwrap();
        assert_eq!(s.cfg.targets[1].label, "Maria");
        assert!(s.cfg.targets[1].sound.is_some());
        let again = Store::open(s.path()).unwrap();
        assert_eq!(again.cfg, s.cfg, "every write is saved");
    }

    #[test]
    fn label_defaults_to_key() {
        let (_d, mut s) = store();
        let t = s.add("  ", "moderator").unwrap();
        assert_eq!(t.label, "moderator");
        s.rename("moderator", "").unwrap();
        assert_eq!(s.cfg.targets[0].label, "moderator");
    }

    #[test]
    fn add_many_reports_skips() {
        let (_d, mut s) = store();
        s.add("", "alpha").unwrap();
        let (added, skipped) =
            s.add_many(&["beta".into(), "alpha".into(), "bad name".into(), "".into()]).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(skipped.len(), 2);
    }

    #[test]
    fn invalid_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, "{nope").unwrap();
        assert!(Store::open(&p).is_err());
    }
}
