//! The keyboard and mouse sheet, shown with F1 or `:keys`, or `?` on the board.
//!
//! A panel at most three quarters of the window each way (smaller when the
//! shortcuts need less), with a header that stays put: the title, how to
//! close it, and a filter that typing fills. Where there is room, the
//! sections are stops on a trail down the left (as on the settings page):
//! a bead rides it as the list scrolls, the section in view is lit, and a
//! click on a stop glides there; the list is one column, and the heading of
//! the section in view docks under the header. In a narrow window the
//! sections flow into one to three columns instead, each to the shortest.
//! What doesn't fit scrolls (wheel, arrows, Page keys, Home/End). A row is
//! what it does, then its keys as keycaps; the words a filter matched are
//! marked in the accent.
//!
//! Motion (docs/DESIGN.md, "Motion on app pages", in the chrome's terms):
//! rows rise into place one after another when the sheet opens, and again,
//! quicker, when the filter changes what is listed. None of it when the
//! desktop asks for less motion.

use std::time::Instant;

use compositor::{Layer, Quad, Renderer, Shape, Text};
use scene::{Rect, Vec2};

use scene::userkeys;

use crate::theme::{rgb, Theme};

/// A section: its title, then (keys, what they do) rows. In keys, " · "
/// separates alternatives, " then " keys one after another, and "+" keys
/// held together; see [`chips`].
pub type Section = (String, Vec<(String, String)>);

/// The sheet's sections: the board's and Page mode's keys as the live
/// tables have them (the key file and `:bind` included, M12 step 10),
/// the user's own keys for the other modes and aliases, then the app's
/// fixed shortcuts and the mouse.
pub fn sections(keys: &userkeys::Keys) -> Vec<Section> {
    let about = |a| keys.about(a);
    let owned = |rows: &[(&str, &str)]| rows.iter().map(|(k, d)| (k.to_string(), d.to_string())).collect::<Vec<_>>();
    let mut board = userkeys::sheet_rows(&keys.board, &about);
    board.extend(owned(&[
        ("Number, then a key", "Do it that many times (3j, 2+)"),
        ("Menu · Shift+F10", "Page menu (j k, Enter)"),
        ("Esc", "Back to the board, then let go"),
    ]));
    let mut page = userkeys::sheet_rows(&keys.page, &about);
    page.push(("Esc".into(), "Out of a field, then back to the board".into()));
    let visual = userkeys::sheet_rows(&keys.visual, &about);
    let mut out: Vec<Section> = vec![
        ("Keyboard".into(), board),
        ("In a page (Enter or Solo)".into(), page),
        ("Caret and Visual (v · V in a page)".into(), visual),
    ];
    for (title, table) in [
        ("Typing in a page", &keys.insert),
        ("While hints show (your keys)", &keys.hints),
        ("In the command line (your keys)", &keys.command),
    ] {
        let rows = userkeys::sheet_rows(table, &about);
        if !rows.is_empty() {
            out.push((title.into(), rows));
        }
    }
    let aliases: Vec<(String, String)> = keys.aliases.iter().map(|(n, t)| (format!(":{n}"), t.to_string())).collect();
    if !aliases.is_empty() {
        out.push(("Your aliases".into(), aliases));
    }
    out.extend(FIXED.iter().map(|(name, rows)| (name.to_string(), owned(rows))));
    out
}

/// Shortcuts outside the key tables, and the mouse.
const FIXED: &[(&str, &[(&str, &str)])] = &[
    (
        "Board",
        &[
            ("Drag board · Scroll", "Pan"),
            ("Ctrl + Scroll", "Zoom"),
            ("Ctrl+K", "Find a card, space or command"),
            ("Ctrl+H", "History (Shift+Del forgets a page)"),
            ("Ctrl+J", "Downloads (Ctrl+Enter shows the folder)"),
            ("Ctrl+,", "Settings"),
            ("Ctrl+T", "New card"),
            ("Alt+PgDn · Alt+PgUp", "Next / previous on the trail"),
            ("Ctrl+S", "Save board or note as Markdown"),
            ("Ctrl+Shift+T", "Reopen closed card"),
            ("Ctrl+Q", "Quit"),
            ("F1 · ? on the board", "Show / hide these shortcuts (also :keys)"),
            (":messages", "Messages shown lately, lines waiting to run"),
            (":keys-file", "Your own keys (then :reload-keys)"),
        ],
    ),
    (
        "Command line (:)",
        &[
            ("↑ · ↓", "Earlier lines starting with what is typed"),
            ("Tab · Shift+Tab", "Walk the list"),
            ("Ctrl+Enter", "Run and keep the line open"),
            ("Ctrl+W · Alt+D", "Delete the word before / after"),
            ("Ctrl+U · Ctrl+K", "Delete to the start / end"),
            ("Ctrl+Y", "Put back what was deleted"),
            ("Ctrl+A · Ctrl+E", "Start / end of the line"),
            ("Alt+B · Alt+F", "A word back / forward"),
            ("Ctrl+Delete", "Forget the highlighted line"),
            ("Number, then :", "A count for the command (3:back)"),
        ],
    ),
    (
        "Cards",
        &[
            ("Click", "Focus a page"),
            ("Drag its label", "Move"),
            ("Drag corner", "Resize"),
            ("Double-click title", "Fill the view"),
            ("× · d", "Close"),
            ("Tab", "Solo (the card fills the window) / back to the board"),
            ("C on a sticky note", "Its next colour"),
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
        "Spaces",
        &[
            ("Ctrl+Shift+N", "New space"),
            ("Ctrl+PgDn · Ctrl+PgUp", "Next / previous space"),
            ("F2", "Rename space"),
            ("Ctrl+Shift+B", "Next tint"),
            ("Ctrl+Shift+P", "Private space on / off"),
            ("Ctrl+Shift+W", "Delete space"),
        ],
    ),
    (
        "Pages",
        &[
            ("Ctrl+L", "Address bar"),
            ("Alt+Left · Alt+Right", "Back / forward"),
            ("Ctrl+R", "Reload"),
            ("Ctrl+F", "Find in page (Enter: next)"),
            ("Esc", "Clear the marks a find left"),
            ("Ctrl++ · Ctrl+− · Ctrl+0", "Zoom page / actual size"),
            ("Ctrl+C · X · V", "Copy / cut / paste"),
            ("Right-click", "Page menu"),
        ],
    ),
];

/// What the sheet keeps between frames: the filter, the scroll, and what the
/// last frame measured (for keys and the wheel, which come between frames).
#[derive(Debug, Default, Clone)]
pub struct Sheet {
    pub filter: String,
    /// How far the sections are scrolled, px, as shown.
    pub scroll: f32,
    /// Where a wheel notch or a key is taking the scroll; `scroll` glides
    /// there, the way the board glides (`scene::scroll::GLIDE_TAU`).
    pub goal: f32,
    /// Touchpad movement, followed at once, and its fling.
    touch: scene::scroll::Pan,
    /// The largest useful scroll, px, as last drawn.
    pub max: f32,
    /// One row and one page of scrolling, px, as last drawn.
    pub line: f32,
    pub page: f32,
    /// The panel, as last drawn: a click outside it closes the sheet.
    pub panel: Rect,
    /// The section stops in the side column, as last drawn: where each is,
    /// and the scroll that brings its section to the top.
    pub stops: Vec<(Rect, f32)>,
    /// When the sheet opened, and when the filter last changed what is
    /// listed: rows arrive from then.
    opened: Option<Instant>,
    changed: Option<Instant>,
    /// The desktop asks for less motion: rows are simply there.
    still: bool,
    /// The stop last clicked, until the list is scrolled some other way:
    /// it stays lit even when its section can't reach the top.
    aim: Option<usize>,
}

impl Sheet {
    /// A sheet opening now, from the top with no filter; `still`: without motion.
    pub fn open(still: bool) -> Sheet {
        Sheet { opened: Some(Instant::now()), still, ..Default::default() }
    }

    /// The stop under `p`, if any.
    pub fn stop_at(&self, p: Vec2) -> Option<usize> {
        self.stops.iter().position(|(r, _)| r.contains(p))
    }

    /// Glides to stop `i`'s section, and keeps that stop lit.
    pub fn go_to_stop(&mut self, i: usize) {
        if let Some(&(_, to)) = self.stops.get(i) {
            self.scroll_to(to);
            self.aim = Some(i);
        }
    }

    /// Glides `px` further (down when positive), within the list; steps
    /// that come quickly add up.
    pub fn scroll_by(&mut self, px: f32) {
        self.scroll_to(self.goal + px);
    }

    /// Glides to `px` from the top, within the list.
    pub fn scroll_to(&mut self, px: f32) {
        self.aim = None;
        self.touch.stop();
        self.goal = px.clamp(0.0, self.max.max(0.0));
    }

    /// Touchpad movement `dy` (winit's sign): followed at once; `ended` is
    /// the fingers lifting, which may fling. `speed` is *Scroll speed*.
    pub fn touch(&mut self, dy: f32, speed: f32, ended: bool) {
        self.aim = None;
        let d = self.touch.touch(scene::Vec2::new(0.0, dy), speed, ended);
        self.move_now(-d.y);
    }

    /// Moves the scroll by `px` at once (no glide).
    fn move_now(&mut self, px: f32) {
        self.scroll = (self.scroll + px).clamp(0.0, self.max.max(0.0));
        self.goal = self.scroll;
    }

    /// Back to the top at once (the filter changed what is listed).
    fn reset(&mut self) {
        self.touch.stop();
        self.scroll = 0.0;
        self.goal = 0.0;
        self.aim = None;
        self.changed = Some(Instant::now());
    }

    /// Advances a glide or a fling by `dt` seconds. Call it every frame the
    /// sheet shows: it also times the touchpad movement a fling comes from.
    pub fn step(&mut self, dt: f32) {
        let fling = self.touch.step(dt);
        if fling.y != 0.0 {
            self.move_now(-fling.y);
            return;
        }
        let left = self.goal - self.scroll;
        // The last half px goes at once rather than creeping.
        if left.abs() < 0.5 {
            self.scroll = self.goal;
        } else {
            self.scroll += left * (1.0 - (-dt / scene::scroll::GLIDE_TAU).exp());
        }
    }

    /// Types `text` into the filter (and goes back to the top).
    pub fn type_text(&mut self, text: &str) {
        self.filter.extend(text.chars().filter(|c| !c.is_control()));
        self.reset();
    }

    pub fn backspace(&mut self) {
        self.filter.pop();
        self.reset();
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.reset();
    }
}

/// One piece of a row's keys: a keycap, or words between and around them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Chip<'a> {
    Cap(&'a str),
    Word(&'a str),
}

/// Mouse actions and the like: words, not keys.
const WORDS: &[&str] = &["Scroll", "Click", "Right-click", "×"];

/// `keys` as keycaps and words: alternatives (" · ") get a dim "or"
/// between them; "a + b" or "a+b" are keys held together; a run of single
/// letters ("h j k l") is a cap each; anything else with spaces, or a mouse
/// word, is a word.
pub fn chips(keys: &str) -> Vec<Chip<'_>> {
    let mut out = Vec::new();
    for (i, alt) in keys.split(" · ").enumerate() {
        if i > 0 {
            out.push(Chip::Word("or"));
        }
        for (j, piece) in alt.split(" then ").enumerate() {
            if j > 0 {
                out.push(Chip::Word("then"));
            }
            piece_chips(piece, &mut out);
        }
    }
    out
}

/// One key (or chord, or run of letters) of an alternative.
fn piece_chips<'a>(alt: &'a str, out: &mut Vec<Chip<'a>>) {
    {
        let parts: Vec<&str> = if alt.trim() == "+" {
            vec!["+"]
        } else {
            alt.split(" + ").flat_map(|p| if p.contains(' ') || p.len() < 2 { vec![p] } else { split_plus(p) }).collect()
        };
        for p in parts {
            let p = p.trim();
            let singles = p.contains(' ') && p.split(' ').all(|w| w.chars().count() <= 1 && !w.is_empty());
            if singles {
                out.extend(p.split(' ').map(Chip::Cap));
            } else if p.contains(' ') || p.contains(',') && p.len() > 2 || WORDS.contains(&p) {
                out.push(Chip::Word(p));
            } else {
                out.push(Chip::Cap(p));
            }
        }
    }
}

/// "Ctrl+Shift+T" → ["Ctrl", "Shift", "T"], keeping a "+" key ("Ctrl++").
fn split_plus(p: &str) -> Vec<&str> {
    match p.strip_suffix("++") {
        Some(rest) => {
            let mut v: Vec<&str> = rest.split('+').collect();
            v.push("+");
            v
        }
        None => p.split('+').filter(|s| !s.is_empty()).collect(),
    }
}

/// The sections and rows `filter` keeps: every word of it must be in the
/// row's keys, what it does, or its section's name (case aside).
pub fn filtered(all: &[Section], filter: &str) -> Vec<Section> {
    let words: Vec<String> = filter.split_whitespace().map(str::to_lowercase).collect();
    all.iter()
        .filter_map(|(name, rows)| {
            let kept: Vec<_> = rows
                .iter()
                .filter(|(k, d)| {
                    let hay = format!("{name} {k} {d}").to_lowercase();
                    words.iter().all(|w| hay.contains(w.as_str()))
                })
                .cloned()
                .collect();
            (!kept.is_empty()).then(|| (name.clone(), kept))
        })
        .collect()
}

/// How many columns of `natural` px (with `gap` between) `avail` px takes: 1 to 3.
pub fn columns(avail: f32, natural: f32, gap: f32) -> usize {
    (((avail + gap) / (natural + gap)).floor() as usize).clamp(1, 3)
}

/// Which column each section (of `heights`) goes in, in order, each to the
/// shortest so far (leftmost on a tie), and the columns' heights.
pub fn flow(heights: &[f32], cols: usize, gap: f32) -> (Vec<usize>, Vec<f32>) {
    let mut tops = vec![0.0f32; cols.max(1)];
    let mut at = Vec::with_capacity(heights.len());
    for h in heights {
        let (c, _) = tops.iter().enumerate().fold((0, f32::MAX), |best, (i, &t)| if t < best.1 - 0.5 { (i, t) } else { best });
        at.push(c);
        tops[c] += if tops[c] > 0.0 { gap } else { 0.0 } + h;
    }
    (at, tops)
}

/// Where the bead stands on the trail of stops (0 = the first section,
/// fractions between): sections start at `tops` (content px, increasing),
/// the list is scrolled `scroll` of at most `max`. A section that can't
/// reach the top counts as reached once the list can scroll no further.
pub fn bead(tops: &[f32], scroll: f32, max: f32) -> f32 {
    let at = |i: usize| tops[i].min(max.max(0.0));
    let Some(i) = (0..tops.len()).rev().find(|&i| at(i) <= scroll + 0.5) else { return 0.0 };
    if i + 1 >= tops.len() {
        return i as f32;
    }
    let span = at(i + 1) - at(i);
    if span <= 0.5 { (i + 1) as f32 } else { i as f32 + ((scroll - at(i)) / span).clamp(0.0, 1.0) }
}

/// `text` cut into runs (byte ranges), each marked whether a word of
/// `filter` matched it there (case aside). Nothing is marked when lower
/// case would change the text's length (offsets would not line up).
pub fn runs(text: &str, filter: &str) -> Vec<(std::ops::Range<usize>, bool)> {
    let lower = text.to_lowercase();
    let mut hit = vec![false; text.len()];
    if lower.len() == text.len() {
        for w in filter.split_whitespace().map(str::to_lowercase) {
            for (at, _) in lower.match_indices(w.as_str()) {
                hit[at..at + w.len()].iter_mut().for_each(|h| *h = true);
            }
        }
    }
    let mut out: Vec<(std::ops::Range<usize>, bool)> = Vec::new();
    for (i, _) in text.char_indices() {
        let end = text[i..].chars().next().map_or(i, |c| i + c.len_utf8());
        match out.last_mut() {
            Some((r, m)) if *m == hit[i] => r.end = end,
            _ => out.push((i..end, hit[i])),
        }
    }
    out
}

/// How far row `k` (in the order drawn) has risen into place, 0 → 1, `t`
/// seconds after the list arrived; `quick` after a filter change.
fn rise(k: usize, t: f32, quick: bool) -> f32 {
    let (step, dur) = if quick { (0.008, 0.14) } else { (0.016, 0.22) };
    crate::chrome::ease_out((t - k.min(16) as f32 * step) / dur)
}

/// The sheet over `screen`, as layers: the panel, the scrolling sections,
/// then the header and edges on top (so a keycap scrolled under them is hidden).
/// `scale` is the window scale times the desktop text size; colours and corners follow `th`.
/// `pointer`: where the pointer is, for the row and stop under it.
pub fn layers(r: &mut Renderer, scale: f32, screen: Rect, th: &Theme, sheet: &mut Sheet, all: &[Section], pointer: Vec2) -> Vec<Layer> {
    let ui = scale;
    let radius = th.radius(ui / th.text_scale);
    let size = crate::chrome::type_::BODY * ui;
    let ks = crate::chrome::type_::CAPTION * ui; // keycap text
    let line = (size * 2.25).max(ks * 1.7 + 8.0 * ui);
    let pad = 24.0 * ui;
    let gap = 28.0 * ui; // between columns
    let section_gap = line * 0.8;
    let title_size = crate::chrome::type_::TITLE * ui;
    let fg = th.text();
    let dim = th.text2();

    let sections = filtered(all, &sheet.filter);

    // Each row's keys, measured once.
    let chip_w = |r: &mut Renderer, c: &Chip| match c {
        Chip::Cap(k) => crate::chrome::keycaps_width(&[r.measure(k, ks, false)], ks),
        Chip::Word(w) => r.measure(w, ks, false),
    };
    let chip_gap = ks * 0.4;
    let measured: Vec<Vec<(Vec<Chip>, f32, f32)>> = sections
        .iter()
        .map(|(_, rows)| {
            rows.iter()
                .map(|(k, d)| {
                    let cs = chips(k);
                    let kw = cs.iter().map(|c| chip_w(r, c)).sum::<f32>() + chip_gap * cs.len().saturating_sub(1) as f32;
                    let dw = r.measure(d, size, false);
                    (cs, kw, dw)
                })
                .collect()
        })
        .collect();

    // Width: three quarters of the window at most. Where a side column of
    // stops leaves room for a column of rows, it has one and the rows are
    // one column; else as many columns as fit their rows' natural width. In
    // a very small window, nearly all of it.
    let max_w = (screen.w * 0.75).max((screen.w - 32.0 * ui).min(360.0 * ui));
    let max_h = (screen.h * 0.75).max((screen.h - 32.0 * ui).min(320.0 * ui));
    let row_gap = 24.0 * ui;
    let natural = all
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(k, d)| {
            let cs = chips(k);
            let kw = cs.iter().map(|c| chip_w(r, c)).sum::<f32>() + chip_gap * cs.len().saturating_sub(1) as f32;
            r.measure(d, size, false) + row_gap + kw
        })
        .fold(0.0, f32::max)
        .clamp(280.0 * ui, 440.0 * ui);
    let rail_w = 160.0 * ui;
    let rail = all.len() >= 3 && max_w - 2.0 * pad - rail_w - gap >= natural;
    let side = if rail { rail_w + gap } else { 0.0 };
    let cols = if rail { 1 } else { columns(max_w - 2.0 * pad, natural, gap) };
    // The columns stretch to the three quarters, but not past half again
    // their natural width (a very wide window gets a narrower sheet).
    let w = max_w.min(2.0 * pad + side + cols as f32 * natural * 1.5 + (cols - 1) as f32 * gap);
    let col_w = ((w - 2.0 * pad - side - (cols - 1) as f32 * gap) / cols as f32).max(1.0);

    // Header: title row, then the filter field, then a hairline.
    let field_h = size * 2.4;
    let header_h = pad + title_size * 1.3 + 14.0 * ui + field_h + 16.0 * ui;
    let heights: Vec<f32> = sections.iter().map(|(_, rows)| line * (rows.len() + 1) as f32).collect();
    let (at, tops) = flow(&heights, cols, section_gap);
    let content_h = tops.iter().copied().fold(0.0, f32::max).max(line * 2.0);
    let foot = pad * 0.75;
    let h = max_h.min(header_h + content_h + 8.0 * ui + foot);
    // The top stays where the whole sheet's is, so filtering shortens the
    // panel from below instead of moving it about.
    let all_heights: Vec<f32> = all.iter().map(|(_, rows)| line * (rows.len() + 1) as f32).collect();
    let all_h = max_h.min(header_h + flow(&all_heights, cols, section_gap).1.iter().copied().fold(0.0, f32::max) + 8.0 * ui + foot);
    let panel = Rect::new(screen.x + (screen.w - w) / 2.0, screen.y + (screen.h - all_h) / 2.0, w, h);
    let body = Rect::new(panel.x, panel.y + header_h, w, (h - header_h - foot).max(1.0));
    let x0 = panel.x + pad + side;

    sheet.max = (content_h + 8.0 * ui - body.h).max(0.0);
    sheet.scroll = sheet.scroll.clamp(0.0, sheet.max);
    sheet.goal = sheet.goal.clamp(0.0, sheet.max);
    sheet.line = line;
    sheet.page = (body.h - line).max(line);
    sheet.panel = panel;

    // How far the rows have come in: since the sheet opened, or (quicker)
    // since the filter changed what is listed.
    let since = sheet.changed.or(sheet.opened).map_or(f32::MAX, |t| t.elapsed().as_secs_f32());
    let quick = sheet.changed.is_some();
    let came = |k: usize| if sheet.still { 1.0 } else { rise(k, since, quick) };
    let hover = th.hover();

    // The panel.
    let mut base = Layer::default();
    base.quads.extend(crate::chrome::scrim(screen, th, 1.0).quads);
    crate::chrome::panel(&mut base, panel, radius, th, ui, crate::chrome::Lift::High, th.panel());

    // The sections, scrolled; each heading and row rises into place in turn.
    let mut rows_l = Layer::default();
    let text = |s: &str, x: f32, y: f32, size: f32, color, bold, clip| Text { text: s.into(), pos: [x, y], size, color, clip, bold };
    let top = body.y + 4.0 * ui - sheet.scroll;
    let mut col_y = vec![0.0f32; cols];
    let mut starts = Vec::with_capacity(sections.len()); // where each section starts, content px
    let hair = crate::chrome::hair(ui);
    let mut k = 0usize; // items drawn, for the stagger
    let push = |rows_l: &mut Layer, mut item: Layer, a: f32| {
        crate::chrome::fade(&mut item, a, Vec2::new(0.0, 8.0 * ui * (1.0 - a)));
        rows_l.quads.extend(item.quads);
        rows_l.texts.extend(item.texts);
    };
    for (i, (name, rows)) in sections.iter().enumerate() {
        let c = at[i];
        let x = x0 + c as f32 * (col_w + gap);
        if col_y[c] > 0.0 {
            col_y[c] += section_gap;
        }
        starts.push(col_y[c]);
        let y0 = top + col_y[c];
        col_y[c] += heights[i];
        if y0 > body.bottom() || y0 + heights[i] < body.y {
            continue;
        }
        let clip = Rect::new(x - 8.0 * ui, body.y, col_w + 8.0 * ui, body.h);
        // A section's name: small spaced capitals, quiet; it only groups.
        let mut item = Layer::default();
        crate::chrome::spaced(&mut item, r, &name.to_uppercase(), x, y0 + line / 2.0 + 3.0 * ui, ks, rgb(th.text2(), 1.0), clip);
        push(&mut rows_l, item, came(k));
        k += 1;
        for (j, (_, desc)) in rows.iter().enumerate() {
            let ry = y0 + (j + 1) as f32 * line;
            if ry > body.bottom() || ry + line < body.y {
                continue;
            }
            let mut item = Layer::default();
            let (cs, kw, _) = &measured[i][j];
            let mid = ry + line / 2.0;
            let row = Rect::new(x - 8.0 * ui, ry, col_w + 16.0 * ui, line);
            if row.contains(pointer) && body.contains(pointer) {
                crate::chrome::fill(&mut item, row, th.small_radius(ui / th.text_scale), rgb(hover, 1.0));
            }
            // A hairline between rows, from the section's title down.
            item.quads.push(Quad {
                shape: Shape::Rect { rect: Rect::new(x, ry, col_w, hair), radius: 0.0 },
                color: rgb(th.line(), 0.8),
                texture: None,
            });
            // Keys right-aligned; what it does gets the rest (cut with "…"),
            // the words a filter matched in the accent.
            // Long lists of keys may take most of the row; they never cross its edge.
            let keys_x = x + col_w - kw.min(col_w * 0.72);
            let clip = Rect::new(x, body.y, col_w, body.h);
            let desc_clip = Rect::new(x, body.y, (keys_x - row_gap * 0.5 - x).max(1.0), body.h);
            let dy = mid - size * 1.3 / 2.0;
            if sheet.filter.trim().is_empty() {
                item.texts.push(text(desc, x, dy, size, rgb(fg, 1.0), false, desc_clip));
            } else {
                for (run, hit) in runs(desc, &sheet.filter) {
                    let dx = r.measure(&desc[..run.start], size, false);
                    let colour = if hit { th.accent_text() } else { fg };
                    item.texts.push(text(&desc[run], x + dx, dy, size, rgb(colour, 1.0), false, desc_clip));
                }
            }
            let mut cx = keys_x;
            for chip in cs {
                match chip {
                    Chip::Cap(key) => {
                        let kw = r.measure(key, ks, false);
                        let right = cx + crate::chrome::keycaps_width(&[kw], ks);
                        crate::chrome::draw_keycaps(&mut item, &[key], &[kw], right, mid, ks, th, ui, clip);
                        cx = right + chip_gap;
                    }
                    Chip::Word(wd) => {
                        item.texts.push(text(wd, cx, mid - ks * 1.3 / 2.0, ks, rgb(dim, 1.0), false, clip));
                        cx += r.measure(wd, ks, false) + chip_gap;
                    }
                }
            }
            push(&mut rows_l, item, came(k));
            k += 1;
        }
    }
    if sections.is_empty() {
        let msg = format!("No shortcuts match \u{201c}{}\u{201d}", sheet.filter.trim());
        let mw = r.measure(&msg, size, false);
        let area = w - 2.0 * pad - side;
        rows_l.texts.push(text(&msg, x0 + (area - mw).max(0.0) / 2.0, body.y + line, size, rgb(dim, 1.0), false, body));
    }

    // On top: the header and the bottom edge in the panel's colour, so
    // rows scrolled under them don't show, then what's in the header.
    let mut top_l = Layer::default();
    let surface = rgb(th.panel(), 1.0);
    let fill = |l: &mut Layer, shape| l.quads.push(Quad { shape, color: surface, texture: None });
    fill(&mut top_l, Shape::Rect { rect: Rect::new(panel.x, panel.y, w, header_h), radius });
    fill(&mut top_l, Shape::Rect { rect: Rect::new(panel.x, panel.y + radius, w, (header_h - radius).max(0.0)), radius: 0.0 });
    fill(&mut top_l, Shape::RoundBottom { rect: Rect::new(panel.x, body.bottom(), w, panel.bottom() - body.bottom()), radius });
    crate::chrome::outline(&mut top_l, panel, radius, hair, rgb(th.line(), 1.0));
    let inner = panel.inflate(-pad);
    let ty = panel.y + pad;
    top_l.texts.push(text("Keyboard shortcuts", inner.x, ty, title_size, rgb(fg, 1.0), true, inner));
    // "Esc to close" at the right of the title, the key as a cap.
    let close = "to close";
    let close_w = r.measure(close, ks, false);
    let esc_w = r.measure("Esc", ks, false);
    let title_mid = ty + title_size * 1.3 / 2.0;
    top_l.texts.push(text(close, inner.right() - close_w, title_mid - ks * 1.3 / 2.0, ks, rgb(dim, 1.0), false, inner));
    crate::chrome::draw_keycaps(&mut top_l, &["Esc"], &[esc_w], inner.right() - close_w - chip_gap, title_mid, ks, th, ui, inner);

    // The filter field.
    let field = Rect::new(inner.x, ty + title_size * 1.3 + 14.0 * ui, inner.w, field_h);
    let rim = if sheet.filter.is_empty() { th.line_strong() } else { th.accent_text() };
    let small = th.small_radius(ui / th.text_scale);
    crate::chrome::fill(&mut top_l, field, small, rgb(th.well(), 1.0));
    crate::chrome::outline(&mut top_l, field, small, hair, rgb(rim, 1.0));
    crate::icons::draw(&mut top_l, r, crate::icons::Icon::Search, field.x + 18.0 * ui, field.center().y, 14.0 * ui, rgb(th.text2(), 1.0));
    let fpad = 34.0 * ui;
    let fy = field.y + (field_h - size * 1.3) / 2.0;
    let total: usize = sections.iter().map(|(_, r)| r.len()).sum();
    let count = if sheet.filter.trim().is_empty() {
        String::new()
    } else {
        format!("{total} {}", if total == 1 { "shortcut" } else { "shortcuts" })
    };
    let count_w = if count.is_empty() { 0.0 } else { r.measure(&count, ks, false) };
    let field_clip = Rect::new(field.x + fpad, field.y, (field.w - fpad - 24.0 * ui - count_w).max(1.0), field_h);
    if sheet.filter.is_empty() {
        top_l.texts.push(text("Type to filter", field.x + fpad, fy, size, rgb(th.text3(), 1.0), false, field_clip));
        top_l.quads.push(Quad {
            shape: Shape::Rect { rect: Rect::new(field.x + fpad - 1.0 * ui, field.y + field_h * 0.25, (1.5 * ui).max(1.0), field_h * 0.5), radius: 0.0 },
            color: rgb(th.accent_text(), 1.0),
            texture: None,
        });
    } else {
        // Typed past the field: keep the end (and the caret) in view.
        let tw = r.measure(&sheet.filter, size, false);
        let x = field.x + fpad - (tw - field_clip.w + 2.0 * ui).max(0.0);
        top_l.texts.push(text(&sheet.filter, x, fy, size, rgb(fg, 1.0), false, field_clip));
        top_l.quads.push(Quad {
            shape: Shape::Rect { rect: Rect::new((x + tw + 1.0 * ui).min(field_clip.right()), field.y + field_h * 0.25, (1.5 * ui).max(1.0), field_h * 0.5), radius: 0.0 },
            color: rgb(th.accent_text(), 1.0),
            texture: None,
        });
        top_l.texts.push(text(&count, field.right() - 12.0 * ui - count_w, field.y + (field_h - ks * 1.3) / 2.0, ks, rgb(dim, 1.0), false, field));
    }
    // A hairline under the header once the rows are under it.
    if sheet.scroll > 0.5 {
        top_l.quads.push(Quad { shape: Shape::Rect { rect: Rect::new(panel.x, body.y - hair, w, hair), radius: 0.0 }, color: rgb(th.divider(), 1.0), texture: None });
    }
    if sheet.max > 0.5 && sheet.scroll < sheet.max - 0.5 {
        top_l.quads.push(Quad { shape: Shape::Rect { rect: Rect::new(panel.x, body.bottom(), w, hair), radius: 0.0 }, color: rgb(th.divider(), 1.0), texture: None });
    }
    // The scrollbar, while there is more than fits.
    if sheet.max > 0.5 {
        let track = Rect::new(panel.right() - pad * 0.5 - 2.0 * ui, body.y + 4.0 * ui, 4.0 * ui, body.h - 8.0 * ui);
        let thumb_h = (track.h * body.h / (body.h + sheet.max)).max(28.0 * ui).min(track.h);
        let thumb_y = track.y + (track.h - thumb_h) * (sheet.scroll / sheet.max);
        top_l.quads.push(Quad {
            shape: Shape::Rect { rect: Rect::new(track.x, thumb_y, track.w, thumb_h), radius: track.w / 2.0 },
            color: rgb(th.line_strong(), 0.9),
            texture: None,
        });
    }
    // The side column: the sections as stops on a trail. The trail fills
    // in the accent down to the bead, which stands where the list is
    // scrolled to; stops passed are filled; the section in view is lit and
    // leans toward the list as the bead reaches it.
    sheet.stops.clear();
    if rail && !sections.is_empty() {
        let rx = panel.x + pad;
        let stop_h = 30.0 * ui;
        let y0 = body.y + 4.0 * ui;
        let n = sections.len();
        // A clicked stop holds the bead, so a section near the end that
        // can't reach the top still shows as the one chosen.
        let f = bead(&starts, sheet.scroll, sheet.max);
        let f = sheet.aim.filter(|&i| i < n).map_or(f, |i| f.min(i as f32));
        let mid = |i: usize| y0 + i as f32 * stop_h + stop_h / 2.0;
        let tx = rx + 5.0 * ui;
        let lw = (1.5 * ui).max(1.0);
        let last = mid(n - 1).min(body.bottom() - stop_h / 2.0);
        let bead_y = (mid(0) + f * stop_h).min(last);
        let mut rail_l = Layer::default();
        crate::chrome::fill(&mut rail_l, Rect::new(tx - lw / 2.0, mid(0), lw, (last - mid(0)).max(0.0)), 0.0, rgb(th.line(), 1.0));
        crate::chrome::fill(&mut rail_l, Rect::new(tx - lw / 2.0, mid(0), lw, (bead_y - mid(0)).max(0.0)), 0.0, rgb(th.accent(), 1.0));
        let clip = Rect::new(rx, body.y, rail_w, body.h);
        for (i, (name, rows)) in sections.iter().enumerate() {
            let m = mid(i);
            if m > body.bottom() - stop_h / 2.0 + 0.5 {
                break;
            }
            let stop = Rect::new(rx - 6.0 * ui, m - stop_h / 2.0, rail_w + 6.0 * ui, stop_h);
            sheet.stops.push((stop, starts[i].min(sheet.max)));
            let mut item = Layer::default();
            let near = (1.0 - (f - i as f32).abs()).clamp(0.0, 1.0);
            let current = i == (f + 0.001).floor() as usize;
            let hovered = stop.contains(pointer);
            if hovered {
                crate::chrome::fill(&mut item, Rect::new(rx + 14.0 * ui, stop.y + 2.0 * ui, rail_w - 14.0 * ui, stop_h - 4.0 * ui), th.small_radius(ui / th.text_scale), rgb(hover, 1.0));
            }
            // The stop: a small dot, filled once passed.
            let d = 6.0 * ui;
            let dot = Rect::new(tx - d / 2.0, m - d / 2.0, d, d);
            let passed = (i as f32) < f - 0.001;
            crate::chrome::fill(&mut item, dot, d / 2.0, rgb(if passed { th.accent() } else { th.panel() }, 1.0));
            crate::chrome::outline(&mut item, dot, d / 2.0, lw, rgb(if passed { th.accent() } else { th.line_strong() }, 1.0));
            let nx = rx + 22.0 * ui + 3.0 * ui * crate::chrome::ease_out(near);
            let colour = if current || hovered { fg } else { th.text2() };
            let count = if sheet.filter.trim().is_empty() { String::new() } else { rows.len().to_string() };
            let cw = if count.is_empty() { 0.0 } else { r.measure(&count, ks, false) + 8.0 * ui };
            let nclip = Rect::new(nx, body.y, (rx + rail_w - nx - cw).max(1.0), body.h);
            item.texts.push(text(name, nx, m - size * 1.3 / 2.0, size, rgb(colour, 1.0), current, nclip));
            if !count.is_empty() {
                item.texts.push(text(&count, rx + rail_w - cw + 8.0 * ui, m - ks * 1.3 / 2.0, ks, rgb(th.text3(), 1.0), false, clip));
            }
            let a = if sheet.still { 1.0 } else { rise(i, since, quick) };
            crate::chrome::fade(&mut item, a, Vec2::new(-6.0 * ui * (1.0 - a), 0.0));
            rail_l.quads.extend(item.quads);
            rail_l.texts.extend(item.texts);
        }
        // The bead, ringed in the panel's colour so it sits on the trail.
        let bd = 10.0 * ui;
        let ring = Rect::new(tx - bd / 2.0 - 2.5 * ui, bead_y - bd / 2.0 - 2.5 * ui, bd + 5.0 * ui, bd + 5.0 * ui);
        crate::chrome::fill(&mut rail_l, ring, ring.w / 2.0, rgb(th.panel(), 1.0));
        crate::chrome::fill(&mut rail_l, Rect::new(tx - bd / 2.0, bead_y - bd / 2.0, bd, bd), bd / 2.0, rgb(th.accent(), 1.0));
        top_l.quads.extend(rail_l.quads);
        top_l.texts.extend(rail_l.texts);

        // The heading of the section in view docks under the header once its
        // own has scrolled away, while rows of it are still in view.
        if let Some(i) = (0..sections.len()).rev().find(|&i| starts[i] < sheet.scroll - 0.5) {
            let end = starts[i] + heights[i];
            if end - sheet.scroll > line * 1.5 {
                let band = Rect::new(x0 - 8.0 * ui, body.y, col_w + 16.0 * ui, line);
                // Pushed up by the next section's heading as it arrives.
                let push_up = (end + section_gap - sheet.scroll - line).min(0.0);
                let band = band.translate(Vec2::new(0.0, push_up));
                let bclip = Rect::new(band.x, body.y, band.w, line);
                let mut dock = Layer::default();
                crate::chrome::fill(&mut dock, band, 0.0, rgb(th.panel(), 1.0));
                crate::chrome::spaced(&mut dock, r, &sections[i].0.to_uppercase(), x0, band.y + line / 2.0 + 3.0 * ui, ks, rgb(th.text2(), 1.0), bclip);
                dock.quads.push(Quad { shape: Shape::Rect { rect: Rect::new(band.x, band.bottom() - hair, band.w, hair), radius: 0.0 }, color: rgb(th.divider(), 1.0), texture: None });
                for q in &mut dock.quads {
                    if let Shape::Rect { rect, .. } = &mut q.shape {
                        let top = rect.y.max(body.y);
                        rect.h = (rect.bottom() - top).max(0.0);
                        rect.y = top;
                    }
                }
                top_l.quads.extend(dock.quads);
                top_l.texts.extend(dock.texts);
            }
        }
    }
    vec![base, rows_l, top_l]
}

#[cfg(test)]
mod tests {
    use super::*;
    use Chip::{Cap, Word};

    #[test]
    fn keys_become_caps_and_words() {
        assert_eq!(chips("Ctrl+Shift+T"), vec![Cap("Ctrl"), Cap("Shift"), Cap("T")]);
        assert_eq!(chips("h j k l · Arrows"), vec![Cap("h"), Cap("j"), Cap("k"), Cap("l"), Word("or"), Cap("Arrows")]);
        assert_eq!(chips("Shift + h j k l"), vec![Cap("Shift"), Cap("h"), Cap("j"), Cap("k"), Cap("l")]);
        assert_eq!(chips("Ctrl + Scroll"), vec![Cap("Ctrl"), Word("Scroll")]);
        assert_eq!(chips("Drag its label"), vec![Word("Drag its label")]);
        assert_eq!(chips("+ · -"), vec![Cap("+"), Word("or"), Cap("-")]);
        assert_eq!(chips("Ctrl+,"), vec![Cap("Ctrl"), Cap(",")]);
        assert_eq!(chips("Ctrl+W then h"), vec![Cap("Ctrl"), Cap("W"), Word("then"), Cap("h")]);
        assert_eq!(chips("m then a…z · ' then A…Z"), vec![Cap("m"), Word("then"), Cap("a…z"), Word("or"), Cap("'"), Word("then"), Cap("A…Z")]);
        assert_eq!(chips("[ · ]"), vec![Cap("["), Word("or"), Cap("]")]);
        assert_eq!(chips("Ctrl++ · Ctrl+−"), vec![Cap("Ctrl"), Cap("+"), Word("or"), Cap("Ctrl"), Cap("−")]);
    }

    #[test]
    fn filter_keeps_matching_rows() {
        let all = sections(&userkeys::Keys::default());
        assert_eq!(filtered(&all, "").len(), all.len());
        let redo = filtered(&all, "redo");
        assert_eq!(redo.len(), 1, "{redo:?}");
        assert_eq!(redo[0].1, vec![("Ctrl+Y · Ctrl+Shift+Z".to_string(), "Redo".to_string())]);
        // Every word must match, in any of keys, text or section.
        assert!(filtered(&all, "ctrl space").iter().all(|(n, rows)| rows.iter().all(|(k, d)| format!("{n} {k} {d}").to_lowercase().contains("space"))));
        assert!(filtered(&all, "nothing like this").is_empty());
        // A section's name keeps all its rows.
        let spaces = all.iter().find(|(n, _)| n == "Spaces").unwrap();
        assert_eq!(filtered(&all, "spaces").iter().find(|(n, _)| n == "Spaces").unwrap().1.len(), spaces.1.len());
    }

    #[test]
    fn sections_follow_the_live_tables() {
        let mut keys = userkeys::Keys::default();
        let all = sections(&keys);
        let page = &all[1].1;
        assert!(page.contains(&("gg".to_string(), "To the top".to_string())));
        assert!(!all.iter().any(|(n, _)| n.contains("your")), "no own keys yet");
        let bind = |modes: &str, k: &str, t: &str| userkeys::Entry::Bind {
            modes: userkeys::modes(modes).unwrap(),
            keys: scene::keys::parse(k).unwrap(),
            target: t.into(),
        };
        keys.apply(&bind("page", "gs", "open https://e.com/{url}")).unwrap();
        keys.apply(&bind("insert", "<C-e>", "scroll-down")).unwrap();
        keys.apply(&userkeys::Entry::Unbind { modes: userkeys::modes("page").unwrap(), keys: scene::keys::parse("gg").unwrap() }).unwrap();
        keys.apply(&userkeys::Entry::Alias { name: "wb".into(), target: "open https://e.com/{url}".into() }).unwrap();
        let all = sections(&keys);
        let page = &all[1].1;
        assert!(page.contains(&("gs".to_string(), ":open https://e.com/{url}".to_string())));
        assert!(!page.iter().any(|(_, d)| d == "To the top"));
        let insert = all.iter().find(|(n, _)| n.starts_with("Typing")).unwrap();
        // A user's binding replaces the default's on its key; the other default stays.
        assert!(insert.1.contains(&("Ctrl+E".to_string(), "Scroll down".to_string())), "{:?}", insert.1);
        assert!(insert.1.iter().any(|(_, d)| d == "Paste the primary selection"), "{:?}", insert.1);
        assert!(!insert.1.iter().any(|(_, d)| d.contains("editor")), "{:?}", insert.1);
        assert!(all.iter().any(|(n, rows)| n == "Your aliases" && rows[0].0 == ":wb"));
    }

    #[test]
    fn scrolling_glides_and_stays_in_the_list() {
        let mut s = Sheet { max: 500.0, ..Default::default() };
        s.scroll_by(100.0);
        s.scroll_by(100.0);
        assert_eq!((s.scroll, s.goal), (0.0, 200.0), "notches add up; nothing moves before a frame");
        s.step(1.0 / 60.0);
        assert!(s.scroll > 0.0 && s.scroll < 100.0, "eases out: {}", s.scroll);
        for _ in 0..60 {
            s.step(1.0 / 60.0);
        }
        assert_eq!(s.scroll, 200.0, "there within a second");
        s.scroll_to(9000.0);
        assert_eq!(s.goal, 500.0);
        s.scroll_by(-9000.0);
        assert_eq!(s.goal, 0.0);
        // The touchpad moves it at once, and typing goes back to the top.
        let mut s = Sheet { max: 500.0, ..Default::default() };
        s.touch(-10.0, 1.0, false);
        assert!(s.scroll > 0.0 && s.goal == s.scroll);
        s.type_text("u");
        assert_eq!((s.scroll, s.goal), (0.0, 0.0));
    }

    #[test]
    fn the_bead_rides_between_sections() {
        let tops = [0.0, 100.0, 300.0, 340.0];
        assert_eq!(bead(&tops, 0.0, 1000.0), 0.0);
        assert!((bead(&tops, 50.0, 1000.0) - 0.5).abs() < 1e-4, "halfway through the first");
        assert!((bead(&tops, 200.0, 1000.0) - 1.5).abs() < 1e-4);
        assert_eq!(bead(&tops, 340.0, 1000.0), 3.0);
        // The last sections can't reach the top: at the end of the list
        // the bead is at the last stop.
        assert_eq!(bead(&tops, 250.0, 250.0), 3.0);
        assert!(bead(&tops, 125.0, 250.0) > 1.0 && bead(&tops, 125.0, 250.0) < 2.0);
        assert_eq!(bead(&[], 10.0, 10.0), 0.0);
    }

    #[test]
    fn filter_words_are_marked_in_runs() {
        let rs = runs("Go to the card above", "card ab");
        let marked: Vec<&str> = rs.iter().filter(|(_, m)| *m).map(|(r, _)| &"Go to the card above"[r.clone()]).collect();
        assert_eq!(marked, ["card", "ab"]);
        // The runs cover the text, in order.
        let joined: String = rs.iter().map(|(r, _)| &"Go to the card above"[r.clone()]).collect();
        assert_eq!(joined, "Go to the card above");
        assert_eq!(runs("Zoom", "").len(), 1);
        assert!(runs("Zoom page / actual size", "ZOOM")[0].1, "case aside");
        assert_eq!(runs("Ünïcödé", "ï").iter().map(|(r, _)| r.end).last(), Some("Ünïcödé".len()));
    }

    #[test]
    fn columns_fit_the_width() {
        assert_eq!(columns(300.0, 400.0, 20.0), 1);
        assert_eq!(columns(820.0, 400.0, 20.0), 2);
        assert_eq!(columns(5000.0, 400.0, 20.0), 3);
    }

    #[test]
    fn sections_flow_to_the_shortest_column() {
        let (at, tops) = flow(&[100.0, 50.0, 30.0, 40.0], 2, 10.0);
        // 100 | 50, then 30 under the 50 (shorter), then 40 there too (90 < 100).
        assert_eq!(at, vec![0, 1, 1, 1]);
        assert_eq!(tops, vec![100.0, 140.0]);
    }
}
