//! The position editor ("setup mode"): the only way to move the overlay.
//! A draggable ghost bar plus a small panel with size, layout, snap buttons,
//! a readout, and Save / Cancel. Nothing is saved until Save.

use crate::overlay::monitor_for;
use crate::state::state;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use vc_core::geometry::{bar_layout, corner_from_xy, window_origin, Item};

pub const GHOST: &str = "editor-ghost";
pub const PANEL: &str = "editor-panel";
const BORDER: i64 = 2;

#[derive(Debug, Clone)]
pub struct Editor {
    pub px: i64,
    pub layout: String,
}

#[derive(Serialize)]
struct GhostCell {
    x: i64,
    y: i64,
    item: Item,
}

#[derive(Serialize)]
pub struct EditorModel {
    px: i64,
    layout: String,
    slot: i64,
    width: i64,
    height: i64,
    cells: Vec<GhostCell>,
    readout: String,
}

fn ghost_bar(app: &AppHandle, e: &Editor) -> (i64, i64, Vec<GhostCell>, i64) {
    let cfg = state(app).cfg();
    let n = cfg.overlay.max_shown.max(1);
    let b = bar_layout(n, e.px, &e.layout);
    let mut items: Vec<Item> = cfg
        .targets
        .iter()
        .take(n as usize)
        .map(|t| Item::Person { key: t.key().into(), label: t.label.clone(), username: t.username.clone().unwrap_or_default() })
        .collect();
    while (items.len() as i64) < n {
        items.push(Item::Empty);
    }
    let cells = b.cells.iter().zip(items).map(|(&(x, y), item)| GhostCell { x, y, item }).collect();
    (b.width, b.height, cells, b.slot)
}

/// Where the bar (inside the ghost's border) is now: monitor, corner, offset.
fn current(app: &AppHandle) -> Option<(tauri::Monitor, String, [i64; 2])> {
    let s = state(app);
    let e = s.ui.lock().unwrap().editor.clone()?;
    let win = app.get_webview_window(GHOST)?;
    let pos = win.outer_position().ok()?;
    let (w, h, _, _) = ghost_bar(app, &e);
    let scale0 = win.scale_factor().unwrap_or(1.0);
    let center = (pos.x as f64 + (w as f64 * scale0) / 2.0, pos.y as f64 + (h as f64 * scale0) / 2.0);
    let mon = app
        .monitor_from_point(center.0, center.1)
        .ok()
        .flatten()
        .or_else(|| monitor_for(app, &s.cfg().overlay.display))?;
    let scale = mon.scale_factor();
    let x = (pos.x - mon.position().x) as f64 / scale + BORDER as f64;
    let y = (pos.y - mon.position().y) as f64 / scale + BORDER as f64;
    let sw = mon.size().width as f64 / scale;
    let sh = mon.size().height as f64 / scale;
    let x = x.clamp(0.0, (sw - w as f64).max(0.0));
    let y = y.clamp(0.0, (sh - h as f64).max(0.0));
    let (corner, offset) = corner_from_xy(x, y, w as f64, h as f64, sw, sh);
    Some((mon, corner, offset))
}

fn readout(corner: &str, offset: [i64; 2]) -> String {
    let words: Vec<&str> = corner.split('-').collect();
    let (v, hz) = (words[0], words[1]);
    let mut text = format!("{v} {hz} corner · {} px from the {hz}, {} px from the {v}", offset[0], offset[1]);
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}

pub fn emit_model(app: &AppHandle) {
    let s = state(app);
    let Some(e) = s.ui.lock().unwrap().editor.clone() else { return };
    let (w, h, cells, slot) = ghost_bar(app, &e);
    let text = current(app).map(|(_, c, o)| readout(&c, o)).unwrap_or_default();
    let m = EditorModel { px: e.px, layout: e.layout.clone(), slot, width: w, height: h, cells, readout: text };
    let _ = app.emit_to(GHOST, "editor", &m);
    let _ = app.emit_to(PANEL, "editor", &m);
}

/// Put the ghost at a corner + offset on a monitor.
fn place(app: &AppHandle, mon: &tauri::Monitor, corner: &str, offset: [i64; 2]) {
    let s = state(app);
    let Some(e) = s.ui.lock().unwrap().editor.clone() else { return };
    let Some(win) = app.get_webview_window(GHOST) else { return };
    let (w, h, _, _) = ghost_bar(app, &e);
    let scale = mon.scale_factor();
    let sw = (mon.size().width as f64 / scale).round() as i64;
    let sh = (mon.size().height as f64 / scale).round() as i64;
    let (x, y) = window_origin(corner, offset, w, h, sw, sh);
    let _ = win.set_size(LogicalSize::new((w + 2 * BORDER) as f64, (h + 2 * BORDER) as f64));
    let _ = win.set_position(PhysicalPosition::new(
        mon.position().x + ((x - BORDER) as f64 * scale).round() as i32,
        mon.position().y + ((y - BORDER) as f64 * scale).round() as i32,
    ));
    emit_model(app);
}

pub fn open(app: &AppHandle) -> Result<(), String> {
    let s = state(app);
    if s.run.lock().unwrap().watching {
        return Err("Stop watching first. The overlay can only be moved in setup mode.".into());
    }
    if let Some(w) = app.get_webview_window(PANEL) {
        let _ = w.set_focus();
        return Ok(());
    }
    let cfg = s.cfg();
    {
        let mut ui = s.ui.lock().unwrap();
        ui.editor_open = true;
        ui.editor = Some(Editor { px: cfg.overlay.avatar_px, layout: cfg.overlay.layout.clone() });
    }
    s.changed();
    WebviewWindowBuilder::new(app, GHOST, WebviewUrl::App("index.html".into()))
        .title("Overlay position")
        .decorations(false)
        .resizable(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .inner_size(100.0, 40.0)
        .build()
        .map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(app, PANEL, WebviewUrl::App("index.html".into()))
        .title("Position the overlay")
        .inner_size(420.0, 380.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .always_on_top(true)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    if let Some(mon) = monitor_for(app, &cfg.overlay.display) {
        place(app, &mon, &cfg.overlay.corner, cfg.overlay.offset);
    }
    Ok(())
}

pub fn update(app: &AppHandle, px: i64, layout: &str) {
    let s = state(app);
    let here = current(app);
    {
        let mut ui = s.ui.lock().unwrap();
        if let Some(e) = ui.editor.as_mut() {
            e.px = px.clamp(8, 48);
            if vc_core::config::LAYOUTS.contains(&layout) {
                e.layout = layout.to_string();
            }
        }
    }
    if let Some((mon, corner, offset)) = here {
        place(app, &mon, &corner, offset); // redraw in place, kept on screen
    }
}

pub fn snap(app: &AppHandle, corner: &str) {
    if !vc_core::config::CORNERS.contains(&corner) {
        return;
    }
    if let Some((mon, _, _)) = current(app) {
        place(app, &mon, corner, [24, 24]);
    }
}

pub fn moved(app: &AppHandle) {
    emit_model(app);
}

pub fn save(app: &AppHandle) -> Result<(), String> {
    let s = state(app);
    let Some((mon, corner, offset)) = current(app) else { return Err("The editor isn't open.".into()) };
    let e = s.ui.lock().unwrap().editor.clone().ok_or("The editor isn't open.")?;
    let primary = app.primary_monitor().ok().flatten();
    let is_primary = primary.is_some_and(|p| p.name() == mon.name() && p.position() == mon.position());
    let display = if is_primary { "primary".to_string() } else { mon.name().cloned().unwrap_or_else(|| "primary".into()) };
    s.store.lock().unwrap().patch(&json!({ "overlay": {
        "corner": corner, "offset": offset, "avatar_px": e.px, "layout": e.layout, "display": display
    }}))?;
    close(app);
    s.toast("ok", "Overlay position saved.");
    Ok(())
}

pub fn close(app: &AppHandle) {
    let s = state(app);
    {
        let mut ui = s.ui.lock().unwrap();
        if !ui.editor_open {
            return;
        }
        ui.editor_open = false;
        ui.editor = None;
    }
    for label in [GHOST, PANEL] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.destroy();
        }
    }
    s.changed();
}
