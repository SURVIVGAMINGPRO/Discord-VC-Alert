// Release builds are a Windows GUI program, so no console window opens
// next to the app (debug builds keep the console for logs).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() { vc_alert_lib::run() }
