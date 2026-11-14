//! Keyboard and mouse hint sheet, shown with `?` when no card is focused.

use compositor::{rgba, Layer, Quad, Renderer, Shape, Text};
use scene::{Rect, Vec2};

/// Section title, then (keys, what they do) rows.
pub const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Board",
        &[
            ("Drag board · Scroll", "Pan"),
            ("Ctrl + Scroll", "Zoom"),
            ("F · Ctrl+0", "Fit everything"),
            ("N · T · Ctrl+T", "New card"),
            ("Ctrl+Z · Ctrl+Shift+Z", "Undo / redo board edits"),
            ("Ctrl+Shift+T", "Reopen closed card"),
            ("?", "Show / hide these hints"),
        ],
    ),
    (
        "Cards",
        &[
            ("Click", "Focus a page"),
            ("Drag title bar", "Move"),
            ("Drag corner", "Resize"),
            ("Double-click title", "Fill the view"),
            ("× · Ctrl+W", "Close"),
            ("Esc", "Leave the page"),
        ],
    ),
    (
        "Selection",
        &[
            ("Shift + drag board", "Select with a band"),
            ("Shift + click title", "Add / remove a card"),
            ("Ctrl+A", "Select all"),
            ("Drag a selected title", "Move the selection"),
            ("Delete", "Close the selection"),
            ("Esc · click board", "Clear the selection"),
        ],
    ),
    (
        "Pages",
        &[
            ("Ctrl+L", "Address bar"),
            ("Alt+Left · Alt+Right", "Back / forward"),
            ("Ctrl+R", "Reload"),
            ("Ctrl+C · X · V", "Copy / cut / paste"),
            ("Right-click", "Page menu"),
        ],
    ),
];

/// Draws the sheet centred in `screen`.
pub fn layer(r: &mut Renderer, scale: f32, screen: Rect) -> Layer {
    let size = 14.0 * scale;
    let line = size * 1.75;
    let pad = 24.0 * scale;
    let gap = 24.0 * scale;
    let keys_w = SECTIONS
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(k, _)| r.measure(k, size, true))
        .fold(0.0, f32::max);
    let desc_w = SECTIONS
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(_, d)| r.measure(d, size, false))
        .fold(0.0, f32::max);
    let rows = SECTIONS.iter().map(|(_, rows)| rows.len() + 1).sum::<usize>() as f32;
    let title = 20.0 * scale;
    let w = (pad * 2.0 + keys_w + gap + desc_w).min(screen.w - 16.0 * scale);
    let h = pad * 2.0 + title * 1.8 + rows * line + (SECTIONS.len() - 1) as f32 * line * 0.5;
    // Two columns of sections if one doesn't fit the height.
    let two = h > screen.h - 32.0 * scale && w * 2.0 + gap <= screen.w;
    let half = SECTIONS.len().div_ceil(2);
    let (w, h) = if two {
        let col_rows = |s: &[(&str, &[(&str, &str)])]| s.iter().map(|(_, r)| r.len() + 1).sum::<usize>() as f32;
        let tallest = col_rows(&SECTIONS[..half]).max(col_rows(&SECTIONS[half..]));
        (w * 2.0 - pad * 2.0 + gap, pad * 2.0 + title * 1.8 + tallest * line + (half - 1) as f32 * line * 0.5)
    } else {
        (w, h)
    };
    let panel = Rect::new(screen.x + (screen.w - w) / 2.0, screen.y + ((screen.h - h) / 2.0).max(8.0), w, h);

    let mut l = Layer::default();
    l.quads.push(Quad { shape: Shape::Rect { rect: screen, radius: 0.0 }, color: [0.0, 0.0, 0.0, 0.45], texture: None });
    l.quads.push(Quad {
        shape: Shape::Shadow { rect: panel.translate(Vec2::new(0.0, 8.0 * scale)), radius: 14.0 * scale, blur: 28.0 * scale },
        color: [0.0, 0.0, 0.0, 0.6],
        texture: None,
    });
    l.quads.push(Quad { shape: Shape::Rect { rect: panel, radius: 14.0 * scale }, color: rgba(36, 36, 44, 1.0), texture: None });
    let text = |s: &str, x: f32, y: f32, size: f32, color, bold| Text { text: s.into(), pos: [x, y], size, color, clip: panel, bold };
    l.texts.push(text("Keyboard shortcuts", panel.x + pad, panel.y + pad, title, rgba(240, 240, 245, 1.0), true));
    let close = "Esc or ? to close";
    let cw = r.measure(close, size * 0.9, false);
    l.texts.push(text(close, panel.right() - pad - cw, panel.y + pad + (title - size * 0.9) * 0.6, size * 0.9, rgba(165, 165, 178, 1.0), false));

    let col_w = keys_w + gap + desc_w;
    let top = panel.y + pad + title * 1.8;
    for (i, (name, rows)) in SECTIONS.iter().enumerate() {
        let (col, first) = if two && i >= half { (1, half) } else { (0, 0) };
        let x = panel.x + pad + col as f32 * (col_w + gap);
        let mut y = top;
        for (_, prev) in &SECTIONS[first..i] {
            y += (prev.len() + 1) as f32 * line + line * 0.5;
        }
        l.texts.push(text(name, x, y, size, rgba(255, 190, 70, 1.0), true));
        for (j, (keys, desc)) in rows.iter().enumerate() {
            let ry = y + (j + 1) as f32 * line;
            l.texts.push(text(keys, x, ry, size, rgba(200, 210, 255, 1.0), true));
            l.texts.push(text(desc, x + keys_w + gap, ry, size, rgba(210, 210, 218, 1.0), false));
        }
    }
    l
}
