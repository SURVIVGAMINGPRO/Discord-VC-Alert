//! The live overlay: a transparent, click-through, always-on-top window that
//! never takes focus. It has no press or drag handling at all (clicks go to the
//! app underneath); hover is detected by polling the cursor position, which
//! works whether or not VC Alert is the active app. Moving it happens only in
//! the position editor.

use crate::state::{state, AppState};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use vc_core::config::Config;
use vc_core::geometry::*;

pub const LABEL: &str = "overlay";
const FLASH_MS: u128 = 1500;

#[derive(Debug, Clone, Serialize)]
pub struct Cell {
    pub x: i64,
    pub y: i64,
    pub item: Item,
    pub bubble: (String, String),
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Model {
    pub style: String,
    pub px: i64,
    pub slot: i64,
    pub opacity: f64,
    pub corner: String,
    pub side: &'static str,
    pub canvas: Canvas,
    pub bar_w: i64,
    pub bar_h: i64,
    pub cells: Vec<Cell>,
    pub count: usize,
    pub count_bubble: (String, String),
    pub flash: Option<String>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(LABEL).is_some() {
        return Ok(());
    }
    let protect = state(app).cfg().overlay.hide_from_capture;
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("VC Alert overlay")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .accept_first_mouse(false)
        .content_protected(protect)
        .visible(false)
        .inner_size(1.0, 1.0)
        .build()?;
    let _ = win.set_ignore_cursor_events(true);
    crate::platform::make_overlay(&win);
    start_hover_loop(app.clone());
    Ok(())
}

/// The monitor the overlay lives on (by name), falling back to the primary.
pub fn monitor_for(app: &AppHandle, display: &str) -> Option<Monitor> {
    if display != "primary" {
        if let Ok(all) = app.available_monitors() {
            if let Some(m) = all.into_iter().find(|m| m.name().is_some_and(|n| n == display)) {
                return Some(m);
            }
        }
    }
    app.primary_monitor().ok().flatten().or_else(|| app.available_monitors().ok()?.into_iter().next())
}

/// v1 `_sync_overlay`: which people (if any) the overlay shows right now.
fn wanted_members(s: &AppState, cfg: &Config) -> Option<Vec<Shown>> {
    let ui = s.ui.lock().unwrap();
    let run = s.run.lock().unwrap();
    let shown_of = |key: &str, username: &str| Shown {
        key: key.to_string(),
        label: cfg.find(key).map(|t| t.label.clone()).unwrap_or_default(),
        username: username.to_string(),
    };
    let (active, members) = if ui.editor_open {
        return None;
    } else if run.watching {
        (!cfg.general.quiet, run.members.iter().map(|m| shown_of(&m.key, &m.username)).collect::<Vec<_>>())
    } else if ui.overlay_preview {
        let n = (cfg.overlay.max_shown + 2) as usize;
        (true, cfg.targets.iter().take(n).map(|t| shown_of(t.key(), t.username.as_deref().unwrap_or(""))).collect())
    } else {
        return None;
    };
    let o = &cfg.overlay;
    (active && o.enabled && (!members.is_empty() || !o.hide_when_empty)).then_some(members)
}

pub fn model(s: &AppState, cfg: &Config, members: &[Shown]) -> Model {
    let o = &cfg.overlay;
    let mut items = shown_items(members, o.max_shown);
    if items.is_empty() {
        items.push(Item::Empty);
    }
    let side = bubble_side(&o.corner, &o.layout);
    let flash = s
        .ui
        .lock()
        .unwrap()
        .flash
        .as_ref()
        .filter(|(_, t)| t.elapsed().as_millis() < FLASH_MS)
        .map(|(k, _)| k.clone());
    let names: Vec<String> = members
        .iter()
        .map(|m| format!("@{}", if m.username.is_empty() { &m.key } else { &m.username }))
        .collect();
    let count_bubble = (
        format!("{} in your channel", members.len()),
        names.iter().take(4).cloned().collect::<Vec<_>>().join(", ") + if names.len() > 4 { "…" } else { "" },
    );

    let (px, bar_w, bar_h, slot, cells) = match o.style.as_str() {
        "count" => {
            let (w, h) = count_bar(o.avatar_px);
            (o.avatar_px, w, h, h - 2 * PAD, Vec::new())
        }
        style => {
            let px = if style == "dots" { dot_px(o.avatar_px) } else { o.avatar_px };
            let b = bar_layout(items.len() as i64, px, &o.layout);
            let mirrored = mirror_cells(&b.cells, b.width, b.height, b.slot, &o.corner);
            (px, b.width, b.height, b.slot, mirrored)
        }
    };
    let canvas = canvas_for(bar_w, bar_h, &o.corner, side);
    let cells = cells
        .into_iter()
        .zip(items)
        .map(|((x, y), item)| {
            let avatar = match &item {
                Item::Person { key, .. } if o.style == "pictures" => s.avatars.data_url(key),
                _ => None,
            };
            Cell { x: canvas.bar_x + x, y: canvas.bar_y + y, bubble: bubble_text(&item, &o.bubble), item, avatar }
        })
        .collect();
    Model {
        style: o.style.clone(),
        px,
        slot,
        opacity: o.opacity,
        corner: o.corner.clone(),
        side,
        canvas,
        bar_w,
        bar_h,
        cells,
        count: members.len(),
        count_bubble,
        flash,
    }
}

/// Recompute, resize, place and show (or hide) the overlay.
pub fn sync(app: &AppHandle) {
    let Some(win) = app.get_webview_window(LABEL) else { return };
    let s = state(app);
    let cfg = s.cfg();
    let _ = win.set_content_protected(cfg.overlay.hide_from_capture);
    let Some(members) = wanted_members(&s, &cfg) else {
        let _ = win.hide();
        s.ui.lock().unwrap().hover = None;
        return;
    };
    let m = model(&s, &cfg, &members);
    let Some(mon) = monitor_for(app, &cfg.overlay.display) else { return };
    let scale = mon.scale_factor();
    let sw = (mon.size().width as f64 / scale).round() as i64;
    let sh = (mon.size().height as f64 / scale).round() as i64;
    let (bx, by) = window_origin(&cfg.overlay.corner, cfg.overlay.offset, m.bar_w, m.bar_h, sw, sh);
    let (cx, cy) = (bx - m.canvas.bar_x, by - m.canvas.bar_y);
    let pos = PhysicalPosition::new(
        mon.position().x + (cx as f64 * scale).round() as i32,
        mon.position().y + (cy as f64 * scale).round() as i32,
    );
    let _ = win.set_size(LogicalSize::new(m.canvas.width as f64, m.canvas.height as f64));
    let _ = win.set_position(pos);
    let _ = app.emit_to(LABEL, "overlay", &m);
    if !win.is_visible().unwrap_or(false) {
        let _ = win.show();
        let _ = win.set_ignore_cursor_events(true);
        crate::platform::make_overlay(&win);
    }
}

/// Hover: poll the cursor (cheap) while the overlay is visible, hit-test the
/// circles, and tell the overlay page which bubble to show. Also re-asserts
/// topmost every 2 s on Windows.
fn start_hover_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticks: u32 = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(60)).await;
            ticks = ticks.wrapping_add(1);
            let Some(win) = app.get_webview_window(LABEL) else { continue };
            if !win.is_visible().unwrap_or(false) {
                continue;
            }
            if ticks % 33 == 0 {
                crate::platform::reassert_topmost(&win);
            }
            let s: Arc<AppState> = state(&app);
            let hit = hit_test(&app, &win, &s);
            let changed = {
                let mut ui = s.ui.lock().unwrap();
                let c = ui.hover != hit;
                ui.hover = hit;
                c
            };
            if changed {
                let _ = app.emit_to(LABEL, "overlay-hover", hit);
            }
        }
    });
}

fn hit_test(app: &AppHandle, win: &tauri::WebviewWindow, s: &AppState) -> Option<usize> {
    let cursor = app.cursor_position().ok()?;
    let pos = win.outer_position().ok()?;
    let scale = win.scale_factor().ok()?;
    let lx = (cursor.x - pos.x as f64) / scale;
    let ly = (cursor.y - pos.y as f64) / scale;
    let cfg = s.cfg();
    let members = wanted_members(s, &cfg)?;
    let m = model_shape(&cfg, &members);
    if m.0 == "count" {
        let (bx, by, bw, bh) = m.1;
        return (lx >= bx as f64 && lx <= (bx + bw) as f64 && ly >= by as f64 && ly <= (by + bh) as f64).then_some(0);
    }
    let r = m.3 as f64 / 2.0 + HOVER_SLACK as f64;
    m.2.iter().position(|&(x, y)| {
        let (cx, cy) = (x as f64 + m.3 as f64 / 2.0, y as f64 + m.3 as f64 / 2.0);
        ((lx - cx).powi(2) + (ly - cy).powi(2)).sqrt() <= r
    })
}

/// Just the geometry of the current model (no avatars), for hit-testing.
fn model_shape(cfg: &Config, members: &[Shown]) -> (String, (i64, i64, i64, i64), Vec<(i64, i64)>, i64) {
    let o = &cfg.overlay;
    let n = shown_items(members, o.max_shown).len().max(1) as i64;
    let side = bubble_side(&o.corner, &o.layout);
    if o.style == "count" {
        let (w, h) = count_bar(o.avatar_px);
        let c = canvas_for(w, h, &o.corner, side);
        return ("count".into(), (c.bar_x, c.bar_y, w, h), Vec::new(), 0);
    }
    let px = if o.style == "dots" { dot_px(o.avatar_px) } else { o.avatar_px };
    let b = bar_layout(n, px, &o.layout);
    let c = canvas_for(b.width, b.height, &o.corner, side);
    let cells = mirror_cells(&b.cells, b.width, b.height, b.slot, &o.corner)
        .into_iter()
        .map(|(x, y)| (c.bar_x + x, c.bar_y + y))
        .collect();
    (o.style.clone(), (c.bar_x, c.bar_y, b.width, b.height), cells, b.slot)
}
