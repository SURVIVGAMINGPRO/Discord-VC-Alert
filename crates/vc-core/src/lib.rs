//! VC Alert engine. No UI code lives here, so everything is unit-testable.

pub mod avatars;
pub mod config;
pub mod discord;
pub mod geometry;
pub mod sound;
pub mod store;
pub mod transfer;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
