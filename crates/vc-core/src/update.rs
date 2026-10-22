//! "Check for updates": only when the user presses the button, ask GitHub for
//! the latest release of VC Alert and, if asked, download this platform's file.
//! Nothing here runs on its own (hard rule 2 allows exactly this).

use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub const REPO: &str = "SURVIVGAMINGPRO/Discord-VC-Alert";
/// The release files the README's download buttons point at (CLAUDE.md § 11).
pub const MAC_ASSET: &str = "VC-Alert-macOS.dmg";
pub const WINDOWS_ASSET: &str = "VC-Alert-Windows-setup.exe";
/// A release file bigger than this isn't ours.
pub const MAX_DOWNLOAD: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// "2.1.4" (the tag without its "v").
    pub version: String,
    /// The release page, for "What's new".
    pub page: String,
    /// (file name, download URL)
    pub assets: Vec<(String, String)>,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&str> {
        self.assets.iter().find(|(n, _)| n == name).map(|(_, u)| u.as_str())
    }
}

/// This platform's file in a release.
pub fn asset_name() -> &'static str {
    if cfg!(windows) { WINDOWS_ASSET } else { MAC_ASSET }
}

/// "2.1.10" is newer than "2.1.9"; anything unreadable is never newer.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> { v.trim().trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect() };
    match (parse(latest), parse(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// GitHub's `releases/latest` answer → the parts we use. Only downloads from
/// GitHub itself are kept.
pub fn parse_latest(v: &Value) -> Result<Release, String> {
    let tag = v.get("tag_name").and_then(Value::as_str).ok_or("GitHub's answer had no version.")?;
    let version = tag.trim().trim_start_matches('v').to_string();
    if !version.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!("GitHub's latest release has an unexpected version ({tag})."));
    }
    let page = v
        .get("html_url")
        .and_then(Value::as_str)
        .filter(|u| u.starts_with(&format!("https://github.com/{REPO}/")))
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{REPO}/releases/latest"));
    let assets = v
        .get("assets")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let name = x.get("name")?.as_str()?.to_string();
                    let url = x.get("browser_download_url")?.as_str()?;
                    url.starts_with(&format!("https://github.com/{REPO}/releases/download/")).then(|| (name, url.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Release { version, page, assets })
}

fn client(timeout: Duration, app_version: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(format!("VCAlert/{app_version}"))
        .build()
        .map_err(|e| e.to_string())
}

/// Ask GitHub for the latest release (one small GET).
pub async fn latest(app_version: &str) -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = client(Duration::from_secs(10), app_version)?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| "Couldn't reach GitHub. Are you online?".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub answered {}. Try again later.", resp.status()));
    }
    let text = resp.text().await.map_err(|_| "GitHub's answer couldn't be read.".to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| "GitHub's answer couldn't be read.".to_string())?;
    parse_latest(&v)
}

/// Download `url` to `dest` (through a `.part` file, so a cut-off download
/// never looks finished).
pub async fn download(url: &str, dest: &Path, app_version: &str) -> Result<(), String> {
    let resp = client(Duration::from_secs(180), app_version)?
        .get(url)
        .send()
        .await
        .map_err(|_| "Couldn't reach GitHub. Are you online?".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("The download failed ({}).", resp.status()));
    }
    if resp.content_length().is_some_and(|n| n as usize > MAX_DOWNLOAD) {
        return Err("That file is too big to be VC Alert.".into());
    }
    let bytes = resp.bytes().await.map_err(|_| "The download was cut off. Try again.".to_string())?;
    if bytes.len() > MAX_DOWNLOAD || bytes.is_empty() {
        return Err("That file doesn't look like VC Alert.".into());
    }
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    std::fs::write(&part, &bytes).map_err(|e| format!("Couldn't save the download: {e}"))?;
    std::fs::rename(&part, dest).map_err(|e| format!("Couldn't save the download: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("2.1.10", "2.1.9"));
        assert!(is_newer("v2.2.0", "2.1.3"));
        assert!(!is_newer("2.1.3", "2.1.3"));
        assert!(!is_newer("2.1.2", "2.1.3"));
        assert!(!is_newer("nonsense", "2.1.3"));
    }

    #[test]
    fn reads_githubs_latest_release_and_keeps_only_our_downloads() {
        let v = json!({
            "tag_name": "v2.1.4",
            "html_url": format!("https://github.com/{REPO}/releases/tag/v2.1.4"),
            "assets": [
                {"name": MAC_ASSET, "browser_download_url": format!("https://github.com/{REPO}/releases/download/v2.1.4/{MAC_ASSET}")},
                {"name": WINDOWS_ASSET, "browser_download_url": "https://elsewhere.example/VC-Alert-Windows-setup.exe"}
            ]
        });
        let r = parse_latest(&v).unwrap();
        assert_eq!(r.version, "2.1.4");
        assert!(r.asset(MAC_ASSET).is_some());
        assert!(r.asset(WINDOWS_ASSET).is_none(), "a download from anywhere but this repository is ignored");
        assert!(parse_latest(&json!({"tag_name": "latest-and-greatest"})).is_err());
        let odd_page = parse_latest(&json!({"tag_name": "v3.0.0", "html_url": "https://phish.example/"})).unwrap();
        assert_eq!(odd_page.page, format!("https://github.com/{REPO}/releases/latest"));
    }
}
