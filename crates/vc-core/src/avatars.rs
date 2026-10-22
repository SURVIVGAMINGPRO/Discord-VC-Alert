//! Profile pictures from Discord's public CDN (v1 `avatars.py`): 128 px PNGs,
//! cached on disk with an `index.json` so people seen before show a real
//! picture straight away. Circle masking and scaling happen in the UI.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const CDN: &str = "https://cdn.discordapp.com";
/// 128 px keeps pictures sharp on Retina screens up to the overlay's 48 px.
pub const SRC_PX: u32 = 128;
pub const MAX_BYTES: usize = 300_000;
pub const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
pub const RETRY_AFTER: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserRef {
    pub id: String,
    pub avatar: Option<String>,
    pub username: String,
}

pub fn default_index(id: &str) -> u64 {
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
        id.parse::<u64>().map(|n| (n >> 22) % 6).unwrap_or(0)
    } else {
        0
    }
}

pub fn avatar_url(u: &UserRef) -> String {
    match &u.avatar {
        Some(h) if !u.id.is_empty() && !h.is_empty() => {
            format!("{CDN}/avatars/{}/{h}.png?size={SRC_PX}", u.id)
        }
        _ => format!("{CDN}/embed/avatars/{}.png", default_index(&u.id)),
    }
}

pub fn avatar_filename(u: &UserRef) -> String {
    match &u.avatar {
        Some(h) if !u.id.is_empty() && !h.is_empty() => format!("{}_{h}.png", u.id),
        _ => format!("default_{}.png", default_index(&u.id)),
    }
}

pub fn is_png(data: &[u8]) -> bool {
    data.len() > 60 && data.starts_with(PNG_MAGIC)
}

/// Only file names we generate ourselves are ever used on disk.
fn safe_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        && !name.contains("..")
}

pub type OnReady = Arc<dyn Fn(String) + Send + Sync>;
/// Runs a download in the background (the app passes its async runtime).
pub type Spawner = Arc<dyn Fn(std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>) + Send + Sync>;

pub struct AvatarCache {
    dir: PathBuf,
    index: Mutex<BTreeMap<String, UserRef>>,
    /// File name being downloaded → everyone waiting for it (people without a
    /// custom picture share Discord's default ones).
    inflight: Mutex<HashMap<String, Vec<String>>>,
    failed: Mutex<HashMap<String, Instant>>,
    on_ready: OnReady,
    spawn: Spawner,
}

impl AvatarCache {
    pub fn new(dir: &Path, on_ready: OnReady, spawn: Spawner) -> Arc<AvatarCache> {
        let index = std::fs::read_to_string(dir.join("index.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Arc::new(AvatarCache {
            dir: dir.to_path_buf(),
            index: Mutex::new(index),
            inflight: Mutex::new(HashMap::new()),
            failed: Mutex::new(HashMap::new()),
            on_ready,
            spawn,
        })
    }

    fn k(key: &str) -> String {
        crate::config::norm_key(key)
    }

    fn save_index(&self) {
        let idx = self.index.lock().unwrap().clone();
        if let Ok(text) = serde_json::to_string(&idx) {
            let _ = crate::store::write_atomic(&self.dir.join("index.json"), text.as_bytes());
        }
    }

    /// Returns true when this person's picture reference changed.
    pub fn remember(&self, key: &str, user: &UserRef) -> bool {
        let changed = {
            let mut idx = self.index.lock().unwrap();
            if idx.get(&Self::k(key)) == Some(user) {
                false
            } else {
                idx.insert(Self::k(key), user.clone());
                true
            }
        };
        if changed {
            self.save_index();
        }
        changed
    }

    /// The cached picture for this person's last-known avatar, if on disk.
    pub fn path_for(&self, key: &str) -> Option<PathBuf> {
        let u = self.index.lock().unwrap().get(&Self::k(key)).cloned()?;
        let name = avatar_filename(&u);
        let p = self.dir.join(&name);
        (safe_name(&name) && p.exists()).then_some(p)
    }

    pub fn data_url(&self, key: &str) -> Option<String> {
        let bytes = std::fs::read(self.path_for(key)?).ok()?;
        is_png(&bytes).then(|| {
            format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
        })
    }

    /// v1 `request`: remember the user; return the file if cached, otherwise
    /// start one background download (with a 60 s back-off after failures).
    pub fn request(self: &Arc<Self>, key: &str, user: &UserRef) -> Option<PathBuf> {
        let changed = self.remember(key, user);
        let name = avatar_filename(user);
        if !safe_name(&name) {
            return None;
        }
        let path = self.dir.join(&name);
        if path.exists() {
            // Already on disk (often a shared default picture): if this person
            // now points at it, tell the UI so their letter becomes the picture.
            if changed {
                (self.on_ready)(key.to_string());
            }
            return Some(path);
        }
        if let Some(t) = self.failed.lock().unwrap().get(&name) {
            if t.elapsed() < RETRY_AFTER {
                return None;
            }
        }
        {
            let mut inflight = self.inflight.lock().unwrap();
            if let Some(waiting) = inflight.get_mut(&name) {
                if !waiting.iter().any(|k| k == key) {
                    waiting.push(key.to_string());
                }
                return None;
            }
            inflight.insert(name.clone(), vec![key.to_string()]);
        }
        let me = self.clone();
        let url = avatar_url(user);
        (self.spawn)(Box::pin(async move {
            let ok = me.download(&url, &path).await;
            let waiting = me.inflight.lock().unwrap().remove(&name).unwrap_or_default();
            if ok {
                for key in waiting {
                    (me.on_ready)(key);
                }
            } else {
                me.failed.lock().unwrap().insert(name, Instant::now());
            }
        }));
        None
    }

    async fn download(&self, url: &str, path: &Path) -> bool {
        let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .user_agent("VCAlert/2")
            .build()
        else {
            return false;
        };
        let Ok(resp) = client.get(url).send().await else { return false };
        if !resp.status().is_success() {
            return false;
        }
        let Ok(bytes) = resp.bytes().await else { return false };
        if bytes.len() > MAX_BYTES || !is_png(&bytes) {
            return false;
        }
        if std::fs::create_dir_all(&self.dir).is_err() {
            return false;
        }
        let mut part = path.as_os_str().to_owned();
        part.push(".part");
        let part = PathBuf::from(part);
        std::fs::write(&part, &bytes).is_ok() && std::fs::rename(&part, path).is_ok()
    }

    /// Delete every cached picture and the index. Returns how many files went.
    pub fn clear(&self) -> usize {
        let mut n = 0;
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for e in entries.flatten() {
                let p = e.path();
                let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
                if ["png", "part", "tmp", "json"].contains(&ext) && std::fs::remove_file(&p).is_ok() {
                    n += 1;
                }
            }
        }
        self.index.lock().unwrap().clear();
        self.failed.lock().unwrap().clear();
        n
    }

    /// Bring over v1's cache (`avatars/` with `index.json`).
    pub fn import_from(&self, old_dir: &Path) -> usize {
        let Ok(text) = std::fs::read_to_string(old_dir.join("index.json")) else { return 0 };
        let Ok(old): Result<BTreeMap<String, UserRef>, _> = serde_json::from_str(&text) else { return 0 };
        let _ = std::fs::create_dir_all(&self.dir);
        let mut n = 0;
        for (k, u) in &old {
            let name = avatar_filename(u);
            if !safe_name(&name) {
                continue;
            }
            let src = old_dir.join(&name);
            if let Ok(bytes) = std::fs::read(&src) {
                if is_png(&bytes) && bytes.len() <= MAX_BYTES && std::fs::write(self.dir.join(&name), bytes).is_ok() {
                    n += 1;
                }
            }
            self.index.lock().unwrap().insert(k.clone(), u.clone());
        }
        self.save_index();
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_and_names() {
        let u = UserRef { id: "123456789012345678".into(), avatar: Some("a45".into()), username: "x".into() };
        assert_eq!(avatar_url(&u), "https://cdn.discordapp.com/avatars/123456789012345678/a45.png?size=128");
        assert_eq!(avatar_filename(&u), "123456789012345678_a45.png");
        let d = UserRef { id: "123456789012345678".into(), avatar: None, username: "x".into() };
        let i = (123456789012345678u64 >> 22) % 6;
        assert_eq!(avatar_url(&d), format!("https://cdn.discordapp.com/embed/avatars/{i}.png"));
        assert_eq!(avatar_filename(&d), format!("default_{i}.png"));
        assert_eq!(default_index("abc"), 0);
        assert!(!is_png(b"GIF89a"));
    }

    #[test]
    fn a_picture_already_on_disk_still_refreshes_the_ui() {
        let dir = tempfile::tempdir().unwrap();
        let ready = Arc::new(Mutex::new(Vec::<String>::new()));
        let r = ready.clone();
        let c = AvatarCache::new(dir.path(), Arc::new(move |k| r.lock().unwrap().push(k)), Arc::new(|_| panic!("no download expected")));
        // Two people without a custom picture share Discord's default one.
        let a = UserRef { id: "4194304".into(), avatar: None, username: "a".into() };
        let b = UserRef { id: "4194304".into(), avatar: None, username: "b".into() };
        let mut png = PNG_MAGIC.to_vec();
        png.resize(100, 0);
        std::fs::write(dir.path().join(avatar_filename(&a)), &png).unwrap();
        assert!(c.request("a", &a).is_some());
        assert!(c.request("b", &b).is_some());
        assert!(c.request("b", &b).is_some(), "same picture again: no repeat refresh");
        assert_eq!(ready.lock().unwrap().as_slice(), ["a", "b"]);
        assert!(c.data_url("b").is_some());
    }

    #[test]
    fn everyone_waiting_on_one_download_is_told() {
        let dir = tempfile::tempdir().unwrap();
        let ready = Arc::new(Mutex::new(Vec::<String>::new()));
        let r = ready.clone();
        let jobs: Arc<Mutex<Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>>>> = Arc::default();
        let j = jobs.clone();
        let c = AvatarCache::new(dir.path(), Arc::new(move |k| r.lock().unwrap().push(k)), Arc::new(move |f| j.lock().unwrap().push(f)));
        let a = UserRef { id: "4194304".into(), avatar: None, username: "a".into() };
        let b = UserRef { id: "4194304".into(), avatar: None, username: "b".into() };
        assert!(c.request("a", &a).is_none());
        assert!(c.request("b", &b).is_none());
        assert_eq!(jobs.lock().unwrap().len(), 1, "one download for the shared file");
        assert_eq!(c.inflight.lock().unwrap().values().next().unwrap(), &vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn clear_removes_files() {
        let dir = tempfile::tempdir().unwrap();
        let c = AvatarCache::new(dir.path(), Arc::new(|_| {}), Arc::new(|f| { std::mem::drop(tokio::spawn(f)); }));
        let u = UserRef { id: "1".into(), avatar: Some("h".into()), username: "u".into() };
        c.remember("u", &u);
        let mut png = PNG_MAGIC.to_vec();
        png.resize(100, 0);
        std::fs::write(dir.path().join("1_h.png"), &png).unwrap();
        assert!(c.path_for("@U").is_some());
        assert!(c.data_url("u").unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(c.clear(), 2);
        assert!(c.path_for("u").is_none());
    }
}
