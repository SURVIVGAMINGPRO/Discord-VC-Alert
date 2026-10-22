//! The Discord client secret and login token, kept in the OS keychain
//! (macOS Keychain / Windows Credential Manager) as one item, so an unsigned
//! build that macOS asks about only asks once.
//!
//! If the keychain can't be used, or `VC_ALERT_SECRETS_FILE=1` is set (for
//! development, where every rebuild would trigger a macOS keychain prompt), a
//! private file in the data folder is used instead.
//!
//! Updates: the item stays where it is, so a new version reads the same
//! secret. An unsigned update looks like a new app to macOS, which may ask
//! once whether it may read the item. If that read fails (Deny, a dismissed
//! prompt), we never treat the secret as missing and never write over it: the
//! failure isn't remembered, a notice explains, and the next use asks again.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use vc_core::discord::Token;

const SERVICE: &str = "VC Alert";
const ACCOUNT: &str = "discord";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Bundle {
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub token: Option<Token>,
}

pub struct Secrets {
    file: PathBuf,
    use_file: bool,
    cache: Mutex<Option<Bundle>>,
    /// Set when we had to fall back to the file.
    pub fallback_note: Mutex<Option<String>>,
    /// Set while the keychain item exists but couldn't be read.
    pub read_note: Mutex<Option<String>>,
}

impl Secrets {
    pub fn new(data_dir: &std::path::Path) -> Secrets {
        let use_file = std::env::var("VC_ALERT_SECRETS_FILE").is_ok_and(|v| v == "1")
            || data_dir.join("secrets.json").exists();
        Secrets {
            file: data_dir.join("secrets.json"),
            use_file,
            cache: Mutex::new(None),
            fallback_note: Mutex::new(None),
            read_note: Mutex::new(None),
        }
    }

    fn entry() -> keyring::Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, ACCOUNT)
    }

    /// The saved bundle. An unreadable keychain item is an error, never an
    /// empty bundle, and isn't cached, so the next call tries again.
    fn load(&self) -> Result<Bundle, String> {
        if let Some(b) = self.cache.lock().unwrap().clone() {
            return Ok(b);
        }
        let b = if self.use_file {
            std::fs::read_to_string(&self.file)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default()
        } else {
            match Self::entry().and_then(|e| e.get_password()) {
                Ok(t) => serde_json::from_str(&t).unwrap_or_default(),
                Err(keyring::Error::NoEntry) => Bundle::default(),
                Err(e) => {
                    eprintln!("keychain read failed: {e}");
                    let note = "VC Alert couldn't read your saved Discord details from the keychain. They're still there: \
                                when macOS asks, choose Always Allow (it asks again next time)."
                        .to_string();
                    *self.read_note.lock().unwrap() = Some(note.clone());
                    return Err(note);
                }
            }
        };
        *self.read_note.lock().unwrap() = None;
        *self.cache.lock().unwrap() = Some(b.clone());
        Ok(b)
    }

    fn store(&self, b: &Bundle) -> Result<(), String> {
        *self.cache.lock().unwrap() = Some(b.clone());
        let text = serde_json::to_string(b).map_err(|e| e.to_string())?;
        if !self.use_file {
            match Self::entry().and_then(|e| e.set_password(&text)) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    *self.fallback_note.lock().unwrap() = Some(format!(
                        "The system keychain couldn't be used ({e}), so the secret is saved in a private file in the app's data folder."
                    ));
                }
            }
        }
        vc_core::store::write_atomic(&self.file, text.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.file, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Like `load`, but after a failed read it doesn't ask the keychain again
    /// (the screen refreshes often, and every ask can be a macOS prompt). Only
    /// Start, Test connection and saving ask again, through `load`.
    fn peek(&self) -> Result<Bundle, String> {
        if let Some(note) = self.read_note.lock().unwrap().clone() {
            if self.cache.lock().unwrap().is_none() {
                return Err(note);
            }
        }
        self.load()
    }

    /// Empty if there's none, or if it couldn't be read (see `readable`).
    pub fn client_secret(&self) -> String {
        self.peek().map(|b| b.client_secret).unwrap_or_default()
    }

    /// True when a secret is saved, and also when the keychain couldn't be
    /// read (it's there, we just weren't allowed to look), so the app doesn't
    /// ask for details the user already gave.
    pub fn has_secret(&self) -> bool {
        self.peek().map(|b| !b.client_secret.is_empty()).unwrap_or(true)
    }

    /// Ok when the saved details can be read right now (may show the macOS
    /// keychain prompt).
    pub fn readable(&self) -> Result<(), String> {
        self.load().map(|_| ())
    }

    pub fn set_client_secret(&self, secret: &str) -> Result<(), String> {
        let mut b = self.load()?;
        if b.client_secret != secret {
            b.client_secret = secret.to_string();
            self.store(&b)?;
        }
        Ok(())
    }

    pub fn token(&self) -> Option<Token> {
        self.peek().ok().and_then(|b| b.token)
    }

    pub fn set_token(&self, t: Option<Token>) -> Result<(), String> {
        let mut b = self.load()?;
        if b.token != t {
            b.token = t;
            self.store(&b)?;
        }
        Ok(())
    }
}
