//! Downloads real pictures from Discord's public CDN. Skipped by default
//! (needs the network); run with:
//!   cargo test -p vc-core --test avatars_live -- --ignored
//! Set VC_ALERT_LIVE_INDEX to an avatars/index.json to also fetch the custom
//! pictures listed there (nothing personal is stored in this repository).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vc_core::avatars::{avatar_url, is_png, AvatarCache, UserRef};

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn downloads_real_pictures() {
    let mut users: Vec<(String, UserRef)> = (0..6u64)
        .map(|i| (format!("default{i}"), UserRef { id: (i << 22).to_string(), avatar: None, username: format!("default{i}") }))
        .collect();
    if let Ok(path) = std::env::var("VC_ALERT_LIVE_INDEX") {
        let idx: BTreeMap<String, UserRef> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        users.extend(idx.into_iter().filter(|(_, u)| u.avatar.is_some()));
    }
    let dir = tempfile::tempdir().unwrap();
    let ready = Arc::new(Mutex::new(Vec::<String>::new()));
    let r = ready.clone();
    let cache = AvatarCache::new(
        dir.path(),
        Arc::new(move |k| r.lock().unwrap().push(k)),
        Arc::new(|f| {
            tokio::spawn(f);
        }),
    );
    for (k, u) in &users {
        cache.request(k, u);
    }
    for _ in 0..100 {
        if ready.lock().unwrap().len() == users.len() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let got = ready.lock().unwrap().clone();
    for (k, u) in &users {
        let path = cache.path_for(k);
        assert!(path.is_some(), "no picture for {k} from {}", avatar_url(u));
        let bytes = std::fs::read(path.unwrap()).unwrap();
        assert!(is_png(&bytes));
        println!("{k}: {} bytes", bytes.len());
    }
    assert_eq!(got.len(), users.len());
}
