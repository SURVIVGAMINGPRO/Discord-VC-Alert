//! Asks GitHub for VC Alert's latest release. Skipped by default (needs the
//! network); run with:
//!   cargo test -p vc-core --test update_live -- --ignored

use vc_core::update::{is_newer, latest, MAC_ASSET, WINDOWS_ASSET};

#[tokio::test]
#[ignore]
async fn reads_the_real_latest_release() {
    let r = latest("test").await.expect("GitHub answered");
    println!("latest: {} ({})", r.version, r.page);
    assert!(!is_newer("0.0.1", &r.version));
    assert!(r.asset(MAC_ASSET).is_some(), "the macOS file is in the release");
    assert!(r.asset(WINDOWS_ASSET).is_some(), "the Windows installer is in the release");
}
