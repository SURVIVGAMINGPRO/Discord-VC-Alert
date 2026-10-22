//! Overlay geometry, ported 1:1 from v1's `overlay.py`, plus the v2 canvas that
//! leaves room for the hover bubble inside the (click-through) overlay window.
//!
//! Key property: cells are mirrored toward the anchoring corner and the window
//! is anchored to that corner, so people already shown never move when someone
//! else joins. The bar grows away from the corner.

use serde::Serialize;

pub const RING: i64 = 2;
pub const PAD: i64 = 2;
pub const HOVER_SLACK: i64 = 2;
/// Space reserved next to the bar for the hover bubble (logical px).
pub const BUBBLE_W: i64 = 240;
pub const BUBBLE_H: i64 = 58;

pub fn gap_for(px: i64) -> i64 {
    (px.div_euclid(6)).max(3)
}

pub fn grid_cols(n: i64, layout: &str) -> i64 {
    let n = n.max(1);
    match layout {
        "column" => 1,
        "grid" => {
            if n <= 2 {
                n
            } else {
                4.min((n + 1) / 2)
            }
        }
        _ => n,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BarLayout {
    pub width: i64,
    pub height: i64,
    pub slot: i64,
    pub cells: Vec<(i64, i64)>,
}

pub fn bar_layout(n: i64, px: i64, layout: &str) -> BarLayout {
    let n = n.max(1);
    let cols = grid_cols(n, layout);
    let rows = (n + cols - 1) / cols;
    let slot = px + 2 * RING;
    let gap = gap_for(px);
    let width = 2 * PAD + cols * slot + (cols - 1) * gap;
    let height = 2 * PAD + rows * slot + (rows - 1) * gap;
    let cells = (0..n)
        .map(|i| (PAD + (i % cols) * (slot + gap), PAD + (i / cols) * (slot + gap)))
        .collect();
    BarLayout { width, height, slot, cells }
}

pub fn mirror_cells(cells: &[(i64, i64)], width: i64, height: i64, slot: i64, corner: &str) -> Vec<(i64, i64)> {
    cells
        .iter()
        .map(|&(mut x, mut y)| {
            if corner.contains("right") {
                x = width - x - slot;
            }
            if corner.starts_with("bottom") {
                y = height - y - slot;
            }
            (x, y)
        })
        .collect()
}

pub fn window_origin(corner: &str, offset: [i64; 2], w: i64, h: i64, sw: i64, sh: i64) -> (i64, i64) {
    let x = if corner.contains("right") { sw - w - offset[0] } else { offset[0] };
    let y = if corner.starts_with("bottom") { sh - h - offset[1] } else { offset[1] };
    (x.min(sw - w).max(0), y.min(sh - h).max(0))
}

pub fn corner_from_xy(x: f64, y: f64, w: f64, h: f64, sw: f64, sh: f64) -> (String, [i64; 2]) {
    let right = x + w / 2.0 > sw / 2.0;
    let bottom = y + h / 2.0 > sh / 2.0;
    let corner = format!("{}-{}", if bottom { "bottom" } else { "top" }, if right { "right" } else { "left" });
    let ox = if right { sw - w - x } else { x };
    let oy = if bottom { sh - h - y } else { y };
    (corner, [(ox.round_ties_even() as i64).max(0), (oy.round_ties_even() as i64).max(0)])
}

pub fn bubble_side(corner: &str, layout: &str) -> &'static str {
    if layout == "column" {
        if corner.contains("right") { "left" } else { "right" }
    } else if corner.starts_with("bottom") {
        "above"
    } else {
        "below"
    }
}

/// Where the bar sits inside the bigger transparent overlay window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Canvas {
    pub width: i64,
    pub height: i64,
    pub bar_x: i64,
    pub bar_y: i64,
}

pub fn canvas_for(bar_w: i64, bar_h: i64, corner: &str, side: &str) -> Canvas {
    let right = corner.contains("right");
    let bottom = corner.starts_with("bottom");
    match side {
        "left" | "right" => {
            let width = bar_w + BUBBLE_W;
            let height = bar_h.max(BUBBLE_H);
            Canvas {
                width,
                height,
                bar_x: if side == "left" { BUBBLE_W } else { 0 },
                bar_y: if bottom { height - bar_h } else { 0 },
            }
        }
        _ => {
            let width = bar_w.max(BUBBLE_W);
            Canvas {
                width,
                height: bar_h + BUBBLE_H,
                bar_x: if right { width - bar_w } else { 0 },
                bar_y: if side == "above" { BUBBLE_H } else { 0 },
            }
        }
    }
}

/// One thing drawn on the bar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Item {
    Person { key: String, label: String, username: String },
    Badge { count: usize, names: Vec<String> },
    Empty,
}

/// Someone in the channel, as far as the overlay cares.
#[derive(Debug, Clone)]
pub struct Shown {
    pub key: String,
    pub label: String,
    pub username: String,
}

/// v1 `shown_items`: the first `max_shown` people in join order, then "+N".
pub fn shown_items(members: &[Shown], max_shown: i64) -> Vec<Item> {
    let k = max_shown.max(1) as usize;
    let mut items: Vec<Item> = members
        .iter()
        .take(k)
        .map(|m| Item::Person {
            key: m.key.clone(),
            label: if m.label.is_empty() {
                if m.username.is_empty() { m.key.clone() } else { m.username.clone() }
            } else {
                m.label.clone()
            },
            username: m.username.clone(),
        })
        .collect();
    if members.len() > k {
        items.push(Item::Badge {
            count: members.len() - k,
            names: members[k..]
                .iter()
                .map(|m| if m.username.is_empty() { m.key.clone() } else { m.username.clone() })
                .collect(),
        });
    }
    items
}

/// v1 `bubble_text`: (main line, second line). Never shows a numeric ID as the
/// name unless no username is known.
pub fn bubble_text(item: &Item, mode: &str) -> (String, String) {
    match item {
        Item::Badge { count, names } => {
            let shown: Vec<String> = names.iter().take(4).map(|n| format!("@{n}")).collect();
            let more = shown.join(", ") + if names.len() > 4 { "…" } else { "" };
            (format!("+{count} more"), more)
        }
        Item::Person { key, label, username } => {
            let name = format!("@{}", if username.is_empty() { key } else { username });
            if mode == "label" {
                return (if label.is_empty() { name } else { label.clone() }, String::new());
            }
            if mode == "both" && !label.is_empty() && label.to_lowercase() != name[1..].to_lowercase() {
                return (label.clone(), name);
            }
            (name, String::new())
        }
        Item::Empty => (String::new(), String::new()),
    }
}

/// Bar size for the compact "count" pill style.
pub fn count_bar(px: i64) -> (i64, i64) {
    let h = (px + 2 * RING).max(18);
    (2 * PAD + (h as f64 * 1.9).round() as i64, 2 * PAD + h)
}

/// Dot diameter for the "dots" style.
pub fn dot_px(px: i64) -> i64 {
    ((px as f64 * 0.4).round() as i64).max(6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        let r = bar_layout(5, 28, "row");
        assert_eq!(r.cells.iter().map(|c| c.1).collect::<std::collections::HashSet<_>>().len(), 1);
        let c = bar_layout(5, 28, "column");
        assert_eq!(c.cells.iter().map(|c| c.0).collect::<std::collections::HashSet<_>>().len(), 1);
        for n in 1..=9 {
            let g = bar_layout(n, 28, "grid");
            let cols = grid_cols(n, "grid");
            assert!(cols <= 4);
            assert!(g.height <= 2 * PAD + 3 * g.slot + 2 * gap_for(28) || n > 8);
        }
        assert_eq!(gap_for(64), 10);
        assert_eq!(gap_for(8), 3);
    }

    #[test]
    fn origin_round_trips() {
        let (sw, sh) = (1920, 1080);
        for corner in crate::config::CORNERS {
            let (w, h) = (100, 40);
            let (x, y) = window_origin(corner, [30, 50], w, h, sw, sh);
            let (c, off) = corner_from_xy(x as f64, y as f64, w as f64, h as f64, sw as f64, sh as f64);
            assert_eq!(c, corner);
            assert_eq!(off, [30, 50]);
        }
    }

    #[test]
    fn nobody_moves_when_someone_joins() {
        let (sw, sh) = (1920, 1080);
        for corner in crate::config::CORNERS {
            for layout in crate::config::LAYOUTS {
                for n in 1..8 {
                    let screen = |count: i64| -> Vec<(i64, i64)> {
                        let b = bar_layout(count, 28, layout);
                        let (ox, oy) = window_origin(corner, [24, 24], b.width, b.height, sw, sh);
                        mirror_cells(&b.cells, b.width, b.height, b.slot, corner)
                            .into_iter()
                            .map(|(x, y)| (ox + x, oy + y))
                            .collect()
                    };
                    let a = screen(n);
                    let b = screen(n + 1);
                    if layout == "grid" && grid_cols(n, "grid") != grid_cols(n + 1, "grid") {
                        continue; // the grid reflows when it gains a column, as in v1
                    }
                    assert_eq!(a[..], b[..n as usize], "{corner} {layout} {n}");
                }
            }
        }
    }

    #[test]
    fn bubble_texts() {
        let p = Item::Person { key: "maria_k".into(), label: "Maria".into(), username: "maria_k".into() };
        assert_eq!(bubble_text(&p, "username"), ("@maria_k".into(), "".into()));
        assert_eq!(bubble_text(&p, "label"), ("Maria".into(), "".into()));
        assert_eq!(bubble_text(&p, "both"), ("Maria".into(), "@maria_k".into()));
        let same = Item::Person { key: "zed".into(), label: "ZED".into(), username: "zed".into() };
        assert_eq!(bubble_text(&same, "both"), ("@zed".into(), "".into()));
        let id = Item::Person { key: "123456789012345678".into(), label: "x".into(), username: "".into() };
        assert_eq!(bubble_text(&id, "username").0, "@123456789012345678");
        let badge = Item::Badge { count: 5, names: (1..=5).map(|i| format!("u{i}")).collect() };
        assert_eq!(bubble_text(&badge, "both"), ("+5 more".into(), "@u1, @u2, @u3, @u4…".into()));
    }

    #[test]
    fn shown_items_badge() {
        let m: Vec<Shown> = (0..5)
            .map(|i| Shown { key: format!("k{i}"), label: String::new(), username: format!("u{i}") })
            .collect();
        let items = shown_items(&m, 3);
        assert_eq!(items.len(), 4);
        assert!(matches!(&items[3], Item::Badge { count: 2, .. }));
    }

    #[test]
    fn canvas_keeps_bar_at_corner() {
        let c = canvas_for(70, 30, "top-right", "below");
        assert_eq!((c.bar_x + 70, c.bar_y), (c.width, 0));
        let c = canvas_for(30, 200, "bottom-right", "left");
        assert_eq!((c.bar_x + 30, c.bar_y + 200), (c.width, c.height));
    }
}
