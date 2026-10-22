//! The Discord client secret and login token, kept in the OS keychain
//! (macOS Keychain / Windows Credential Manager) as one item, so an unsigned
//! build that macOS asks about only asks once.
//!
//! If the keychain can't be used, or `VC_ALERT_SECRETS_FILE=1` is set (for
//! development, where every rebuild would trigger a macOS keychain prompt), a
//! private file in the data folder is used instead.

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
        }
    }

    fn entry() -> keyring::Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, ACCOUNT)
    }

    fn load(&self) -> Bundle {
        if let Some(b) = self.cache.lock().unwrap().clone() {
            return b;
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
                    Bundle::default()
                }
            }
        };
        *self.cache.lock().unwrap() = Some(b.clone());
        b
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

    pub fn client_secret(&self) -> String {
        self.load().client_secret
    }

    pub fn has_secret(&self) -> bool {
        !self.load().client_secret.is_empty()
    }

    pub fn set_client_secret(&self, secret: &str) -> Result<(), String> {
        let mut b = self.load();
        if b.client_secret != secret {
            b.client_secret = secret.to_string();
            self.store(&b)?;
        }
        Ok(())
    }

    pub fn token(&self) -> Option<Token> {
        self.load().token
    }

    pub fn set_token(&self, t: Option<Token>) -> Result<(), String> {
        let mut b = self.load();
        if b.token != t {
            b.token = t;
            self.store(&b)?;
        }
        Ok(())
    }
}
