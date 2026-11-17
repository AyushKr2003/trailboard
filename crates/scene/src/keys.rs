//! Key bindings: chords, key sequences and counts (M12). Our own design; the
//! written form of keys follows the common vim-style notation (`gg`, `<C-d>`,
//! `<S-Tab>`, `<Esc>`), which users of keyboard-driven tools already know.
//!
//! A [`Bindings`] table maps sequences of [`Chord`]s to actions. [`Pending`]
//! takes keys one at a time: digits before a sequence make a count (`5j`), a
//! key that starts a longer binding waits for the next one, and a sequence
//! that is both a binding and the start of a longer one runs after
//! [`TIMEOUT`] without another key.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::{Duration, Instant};

use crate::nav::Dir;

/// How long a half-typed sequence waits for its next key.
pub const TIMEOUT: Duration = Duration::from_millis(1000);

/// The largest count kept; more digits don't raise it.
pub const MAX_COUNT: u32 = 9999;

/// One key, as the layout means it. Printable keys are their character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Esc,
    Enter,
    Tab,
    Space,
    Backspace,
    Delete,
    Insert,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// F1–F12.
    F(u8),
    /// The Menu (context menu) key.
    Menu,
}

/// A key with its modifiers. For a character without Ctrl or Alt, Shift is
/// part of the character (`G`, `$`, `?`) and `shift` is false; with Ctrl or
/// Alt, a letter is lower case and `shift` says whether Shift was held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Chord {
    /// A chord in its one normal form (see [`Chord`]).
    pub fn new(key: Key, ctrl: bool, alt: bool, shift: bool) -> Chord {
        match key {
            Key::Char(c) if !ctrl && !alt => Chord { key: Key::Char(c), ctrl, alt, shift: false },
            Key::Char(c) if c.is_ascii_uppercase() => Chord { key: Key::Char(c.to_ascii_lowercase()), ctrl, alt, shift: true },
            _ => Chord { key, ctrl, alt, shift },
        }
    }

    /// A plain key without modifiers.
    pub fn plain(key: Key) -> Chord {
        Chord::new(key, false, false, false)
    }

    /// No Ctrl or Alt (Shift may be part of the key).
    pub fn is_bare(&self) -> bool {
        !self.ctrl && !self.alt
    }

    fn digit(&self) -> Option<u32> {
        match self.key {
            Key::Char(c) if self.is_bare() => c.to_digit(10),
            _ => None,
        }
    }
}

/// Names for keys inside `<…>`, as written and as shown.
const NAMES: &[(&str, Key)] = &[
    ("Esc", Key::Esc),
    ("Enter", Key::Enter),
    ("Tab", Key::Tab),
    ("Space", Key::Space),
    ("BS", Key::Backspace),
    ("Del", Key::Delete),
    ("Ins", Key::Insert),
    ("Left", Key::Left),
    ("Right", Key::Right),
    ("Up", Key::Up),
    ("Down", Key::Down),
    ("Home", Key::Home),
    ("End", Key::End),
    ("PageUp", Key::PageUp),
    ("PageDown", Key::PageDown),
    ("Menu", Key::Menu),
    ("lt", Key::Char('<')),
];

/// Other spellings accepted when reading.
const ALIASES: &[(&str, Key)] = &[
    ("Escape", Key::Esc),
    ("CR", Key::Enter),
    ("Return", Key::Enter),
    ("Backspace", Key::Backspace),
    ("Delete", Key::Delete),
    ("Insert", Key::Insert),
    ("PgUp", Key::PageUp),
    ("PgDn", Key::PageDown),
];

fn key_by_name(name: &str) -> Option<Key> {
    if let Some(n) = name.strip_prefix(['F', 'f']).and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=12).contains(n)) {
        return Some(Key::F(n));
    }
    NAMES.iter().chain(ALIASES).find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, k)| *k)
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.key {
            Key::F(n) => Some(format!("F{n}")),
            Key::Char('<') => Some("lt".to_string()),
            Key::Char(c) if c != ' ' => None,
            k => NAMES.iter().find(|(_, x)| *x == k).map(|(n, _)| n.to_string()),
        };
        let mods = format!(
            "{}{}{}",
            if self.ctrl { "C-" } else { "" },
            if self.alt { "A-" } else { "" },
            if self.shift { "S-" } else { "" }
        );
        match (name, self.key) {
            (None, Key::Char(c)) if mods.is_empty() => write!(f, "{c}"),
            (None, Key::Char(c)) => write!(f, "<{mods}{c}>"),
            (Some(n), _) => write!(f, "<{mods}{n}>"),
            (None, _) => Ok(()),
        }
    }
}

/// Reads a written sequence: characters stand for themselves, `<…>` holds a
/// named key or a chord (`<C-d>`, `<A-Left>`, `<S-Tab>`, `<lt>` for `<`).
pub fn parse(seq: &str) -> Result<Vec<Chord>, String> {
    let mut out = Vec::new();
    let mut rest = seq;
    while let Some(c) = rest.chars().next() {
        if c == '<'
            && let Some(end) = rest.find('>').filter(|&e| e > 1)
        {
            out.push(parse_bracket(&rest[1..end]).ok_or_else(|| format!("unknown key <{}> in {seq:?}", &rest[1..end]))?);
            rest = &rest[end + 1..];
        } else {
            out.push(Chord::plain(if c == ' ' { Key::Space } else { Key::Char(c) }));
            rest = &rest[c.len_utf8()..];
        }
    }
    if out.is_empty() {
        return Err("empty key sequence".into());
    }
    Ok(out)
}

fn parse_bracket(inner: &str) -> Option<Chord> {
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    let mut rest = inner;
    // Modifiers are one letter and a dash; a lone `-` (as in `<C-->`) is the key.
    while rest.len() > 2 && rest.as_bytes()[1] == b'-' {
        match rest.as_bytes()[0].to_ascii_uppercase() {
            b'C' => ctrl = true,
            b'A' | b'M' => alt = true,
            b'S' => shift = true,
            _ => return None,
        }
        rest = &rest[2..];
    }
    let key = match key_by_name(rest) {
        Some(k) => k,
        None => {
            let mut cs = rest.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) => Key::Char(c),
                _ => return None,
            }
        }
    };
    // `<S-g>` is `G`.
    let key = match key {
        Key::Char(c) if shift && !ctrl && !alt => {
            shift = false;
            Key::Char(c.to_ascii_uppercase())
        }
        k => k,
    };
    Some(Chord::new(key, ctrl, alt, shift))
}

/// Writes a sequence back in the notation [`parse`] reads.
pub fn display(seq: &[Chord]) -> String {
    seq.iter().map(Chord::to_string).collect()
}

/// What a sequence is in a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup<A> {
    /// Bound, and no longer binding starts with it.
    Exact(A),
    /// Only the start of longer bindings.
    Prefix,
    /// Bound, and also the start of longer bindings.
    ExactAndPrefix(A),
    None,
}

/// A table of key sequences to actions.
#[derive(Debug, Clone)]
pub struct Bindings<A> {
    map: HashMap<Vec<Chord>, A>,
    prefixes: HashSet<Vec<Chord>>,
}

impl<A> Default for Bindings<A> {
    fn default() -> Self {
        Bindings { map: HashMap::new(), prefixes: HashSet::new() }
    }
}

impl<A: Copy> Bindings<A> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a written sequence (see [`parse`]); a later binding of the same
    /// sequence replaces the earlier one.
    pub fn bind(&mut self, seq: &str, action: A) -> Result<(), String> {
        let chords = parse(seq)?;
        for n in 1..chords.len() {
            self.prefixes.insert(chords[..n].to_vec());
        }
        self.map.insert(chords, action);
        Ok(())
    }

    pub fn lookup(&self, seq: &[Chord]) -> Lookup<A> {
        match (self.map.get(seq), self.prefixes.contains(seq)) {
            (Some(a), false) => Lookup::Exact(*a),
            (Some(a), true) => Lookup::ExactAndPrefix(*a),
            (None, true) => Lookup::Prefix,
            (None, false) => Lookup::None,
        }
    }

    /// Every binding, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (&[Chord], A)> {
        self.map.iter().map(|(k, a)| (k.as_slice(), *a))
    }
}

/// What a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome<A> {
    /// Run the action, `count` times if given.
    Run { action: A, count: Option<u32> },
    /// The key was taken; more are expected.
    Wait,
    /// Esc dropped a half-typed count or sequence.
    Cancelled,
    /// Not a binding. `swallow`: it ended a half-typed count or sequence and
    /// is a bare key, so it does nothing else (a Ctrl or Alt chord still goes
    /// on to the app's other shortcuts).
    Unbound { swallow: bool },
}

/// Keys typed so far toward a binding.
#[derive(Debug, Clone, Default)]
pub struct Pending {
    count: Option<u32>,
    keys: Vec<Chord>,
    /// When the last key of an unfinished sequence came.
    at: Option<Instant>,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        self.count.is_none() && self.keys.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Pending::default();
    }

    /// What has been typed, for showing (`5g`).
    pub fn display(&self) -> String {
        let count = self.count.map(|c| c.to_string()).unwrap_or_default();
        count + &display(&self.keys)
    }

    /// Takes one key. `counts`: digits before a sequence make a count.
    pub fn feed<A: Copy>(&mut self, table: &Bindings<A>, chord: Chord, now: Instant, counts: bool) -> Outcome<A> {
        let was_pending = !self.is_empty();
        if chord == Chord::plain(Key::Esc) && was_pending {
            self.clear();
            return Outcome::Cancelled;
        }
        // A count comes before the sequence; 0 only continues one (alone it
        // may be a key of its own).
        if counts
            && self.keys.is_empty()
            && let Some(d) = chord.digit()
            && (d != 0 || self.count.is_some())
        {
            self.count = Some(self.count.unwrap_or(0).saturating_mul(10).saturating_add(d).min(MAX_COUNT));
            return Outcome::Wait;
        }
        self.keys.push(chord);
        match table.lookup(&self.keys) {
            Lookup::Exact(action) => {
                let count = self.count;
                self.clear();
                Outcome::Run { action, count }
            }
            Lookup::Prefix | Lookup::ExactAndPrefix(_) => {
                self.at = Some(now);
                Outcome::Wait
            }
            Lookup::None => {
                self.clear();
                Outcome::Unbound { swallow: was_pending && chord.is_bare() }
            }
        }
    }

    /// Ends a sequence that has waited [`TIMEOUT`]: one that is a binding by
    /// itself runs; a mere start is dropped. A lone count keeps waiting.
    pub fn expire<A: Copy>(&mut self, table: &Bindings<A>, now: Instant) -> Option<Outcome<A>> {
        let at = self.at?;
        if now.duration_since(at) < TIMEOUT {
            return None;
        }
        let out = match table.lookup(&self.keys) {
            Lookup::ExactAndPrefix(action) | Lookup::Exact(action) => Outcome::Run { action, count: self.count },
            _ => Outcome::Cancelled,
        };
        self.clear();
        Some(out)
    }
}

/// What a key can do. Board actions act on the current card (or the
/// selection), as the keys always did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Make the nearest card that way current.
    Move(Dir),
    /// Pan the board a quarter window.
    Pan(Dir),
    ZoomIn,
    ZoomOut,
    /// Give the current card the keys (or edit a note).
    Enter,
    /// Close the selection, else the current card.
    Close,
    Quit,
    Fit,
    LinkHints,
    Tidy,
    Arrange,
    NewNote,
    NewSticky,
    Sidebar,
    /// Select or deselect the current card.
    Select,
    SelectAll,
    /// The current card fills the view.
    Fill,
    ScrollDown,
    ScrollUp,
    NameTrail,
    TrailColour,
    Fold,
    Outline,
    StackPrev,
    StackNext,
    Undo,
    Redo,
    // Page mode (a card entered, or Solo).
    /// Scroll the page a little that way.
    Scroll(Dir),
    /// To the top / bottom / left edge / right edge.
    Top,
    Bottom,
    LeftEdge,
    RightEdge,
    /// Link hints that follow in the same card (`LinkHints` opens beside).
    LinkHintsHere,
    Back,
    Forward,
    Reload,
    /// Reload, skipping the cache.
    ReloadFresh,
    /// All keys to the page until Esc.
    Insert,
    /// Focus the first text field and type in it.
    FocusField,
    CopyUrl,
    CopyTitle,
    /// Copy `[title](url)`.
    CopyMarkdown,
    /// Open the clipboard's address (or search for its text) in this card / beside.
    PasteHere,
    PasteBeside,
    AddressBar,
    /// An empty address bar whose Enter opens beside the card.
    AddressBarBeside,
    Find,
    FindNext,
    FindPrev,
    /// All keys to the page, Esc included, until Shift+Esc.
    Passthrough,
    /// Label the page's scroll sections to pick one (`s`).
    Sections,
    /// The next / previous scroll section in reading order (`]s` / `[s`).
    NextSection,
    PrevSection,
    /// The section around the scroll target (`S`).
    OuterSection,
}

impl Action {
    /// A count repeats it (`3j` moves three cards).
    pub fn repeats(self) -> bool {
        matches!(
            self,
            Action::Move(_)
                | Action::Pan(_)
                | Action::ZoomIn
                | Action::ZoomOut
                | Action::ScrollDown
                | Action::ScrollUp
                | Action::Scroll(_)
                | Action::Back
                | Action::Forward
                | Action::FindNext
                | Action::FindPrev
        )
    }
}

/// The board's keys while no page has them: the keys of M5/M6, unchanged.
pub fn board() -> Bindings<Action> {
    use Action::*;
    let mut b = Bindings::new();
    let table: &[(&str, Action)] = &[
        ("h", Move(Dir::Left)),
        ("j", Move(Dir::Down)),
        ("k", Move(Dir::Up)),
        ("l", Move(Dir::Right)),
        ("<Left>", Move(Dir::Left)),
        ("<Down>", Move(Dir::Down)),
        ("<Up>", Move(Dir::Up)),
        ("<Right>", Move(Dir::Right)),
        ("H", Pan(Dir::Left)),
        ("J", Pan(Dir::Down)),
        ("K", Pan(Dir::Up)),
        ("L", Pan(Dir::Right)),
        ("<S-Left>", Pan(Dir::Left)),
        ("<S-Down>", Pan(Dir::Down)),
        ("<S-Up>", Pan(Dir::Up)),
        ("<S-Right>", Pan(Dir::Right)),
        // The = key zooms in with or without Shift, the - key out.
        ("=", ZoomIn),
        ("+", ZoomIn),
        ("-", ZoomOut),
        ("_", ZoomOut),
        ("<Enter>", Enter),
        ("<S-Enter>", Enter),
        ("<Space>", Enter),
        ("<S-Space>", Enter),
        ("<Del>", Close),
        ("<S-Del>", Close),
        ("<BS>", Close),
        ("<S-BS>", Close),
        ("q", Quit),
        ("F", Fit),
        ("f", LinkHints),
        ("t", Tidy),
        ("T", Tidy),
        ("m", Arrange),
        ("n", NewNote),
        ("N", NewSticky),
        ("b", Sidebar),
        ("v", Select),
        ("<C-a>", SelectAll),
        ("z", Fill),
        ("d", ScrollDown),
        ("u", ScrollUp),
        ("r", NameTrail),
        ("c", TrailColour),
        ("s", Fold),
        ("S", Fold),
        ("o", Outline),
        // The bracket keys, with or without Shift.
        ("[", StackPrev),
        ("{", StackPrev),
        ("]", StackNext),
        ("}", StackNext),
        ("<C-z>", Undo),
        ("<C-S-z>", Redo),
        ("<C-y>", Redo),
        ("<C-S-y>", Redo),
    ];
    for (seq, a) in table {
        b.bind(seq, *a).expect("board keys parse");
    }
    b
}

/// Page mode's keys: a card is entered (Enter, or Solo) and our keys come
/// first. Keys not here that type nothing (Enter, Tab, arrows, Space, Page
/// Up/Down…) and Ctrl/Alt shortcuts the app doesn't use go on to the page,
/// so its own keyboard use (focus moving, buttons, video) keeps working.
/// Owner's choices (2026-11-17): f follows in the same card, F beside; d/u
/// scroll; Ctrl+F stays Find, so Ctrl+F/Ctrl+B don't page.
pub fn page() -> Bindings<Action> {
    use Action::*;
    let mut b = Bindings::new();
    let table: &[(&str, Action)] = &[
        ("j", Scroll(Dir::Down)),
        ("k", Scroll(Dir::Up)),
        ("h", Scroll(Dir::Left)),
        ("l", Scroll(Dir::Right)),
        ("d", ScrollDown),
        ("u", ScrollUp),
        ("<C-d>", ScrollDown),
        ("<C-u>", ScrollUp),
        ("gg", Top),
        ("G", Bottom),
        ("0", LeftEdge),
        ("$", RightEdge),
        ("f", LinkHintsHere),
        ("F", LinkHints),
        ("H", Back),
        ("L", Forward),
        ("r", Reload),
        ("R", ReloadFresh),
        ("i", Insert),
        ("gi", FocusField),
        ("yy", CopyUrl),
        ("yt", CopyTitle),
        ("ym", CopyMarkdown),
        ("p", PasteHere),
        ("P", PasteBeside),
        ("o", AddressBar),
        ("O", AddressBarBeside),
        ("/", Find),
        ("n", FindNext),
        ("N", FindPrev),
        ("<C-v>", Passthrough),
        ("s", Sections),
        ("]s", NextSection),
        ("[s", PrevSection),
        ("S", OuterSection),
    ];
    for (seq, a) in table {
        b.bind(seq, *a).expect("page keys parse");
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(c: char) -> Chord {
        Chord::plain(Key::Char(c))
    }

    #[test]
    fn notation_round_trips() {
        for s in ["gg", "G", "<C-d>", "<S-Tab>", "<Esc>", "<C-S-z>", "<A-Left>", "<lt>", "<F5>", "<Space>", "$", "]s"] {
            assert_eq!(display(&parse(s).unwrap()), s, "{s}");
        }
        assert_eq!(parse("<S-g>").unwrap(), vec![ch('G')]);
        assert_eq!(parse("<C-D>").unwrap(), parse("<C-S-d>").unwrap());
        assert_eq!(parse("<CR>").unwrap(), parse("<Enter>").unwrap());
        assert_eq!(parse("<C-->").unwrap(), vec![Chord::new(Key::Char('-'), true, false, false)]);
        assert_eq!(parse("<").unwrap(), vec![ch('<')]);
        assert!(parse("").is_err());
        assert!(parse("<Nope>").is_err());
        assert!(parse("<X-a>").is_err());
    }

    #[test]
    fn shift_is_part_of_a_bare_character() {
        assert_eq!(Chord::new(Key::Char('G'), false, false, true), ch('G'));
        assert_eq!(Chord::new(Key::Char('Z'), true, false, false), Chord::new(Key::Char('z'), true, false, true));
        assert_eq!(parse("<S-Tab>").unwrap(), vec![Chord::new(Key::Tab, false, false, true)]);
    }

    fn table() -> Bindings<u8> {
        let mut b = Bindings::new();
        b.bind("j", 1).unwrap();
        b.bind("gg", 2).unwrap();
        b.bind("g", 3).unwrap();
        b.bind("yy", 4).unwrap();
        b.bind("0", 5).unwrap();
        b.bind("<C-d>", 6).unwrap();
        b
    }

    #[test]
    fn lookup_kinds() {
        let b = table();
        assert_eq!(b.lookup(&[ch('j')]), Lookup::Exact(1));
        assert_eq!(b.lookup(&[ch('g')]), Lookup::ExactAndPrefix(3));
        assert_eq!(b.lookup(&[ch('y')]), Lookup::Prefix);
        assert_eq!(b.lookup(&[ch('x')]), Lookup::None);
    }

    #[test]
    fn counts_and_sequences() {
        let b = table();
        let t = Instant::now();
        let mut p = Pending::default();
        assert_eq!(p.feed(&b, ch('1'), t, true), Outcome::Wait);
        assert_eq!(p.feed(&b, ch('0'), t, true), Outcome::Wait);
        assert_eq!(p.display(), "10");
        assert_eq!(p.feed(&b, ch('j'), t, true), Outcome::Run { action: 1, count: Some(10) });
        assert!(p.is_empty());
        // 0 alone is a key.
        assert_eq!(p.feed(&b, ch('0'), t, true), Outcome::Run { action: 5, count: None });
        // Without counts, digits are keys.
        assert_eq!(p.feed(&b, ch('1'), t, false), Outcome::Unbound { swallow: false });
        // A sequence.
        assert_eq!(p.feed(&b, ch('3'), t, true), Outcome::Wait);
        assert_eq!(p.feed(&b, ch('y'), t, true), Outcome::Wait);
        assert_eq!(p.display(), "3y");
        assert_eq!(p.feed(&b, ch('y'), t, true), Outcome::Run { action: 4, count: Some(3) });
        // Ctrl chords bind too.
        let cd = Chord::new(Key::Char('d'), true, false, false);
        assert_eq!(p.feed(&b, cd, t, true), Outcome::Run { action: 6, count: None });
    }

    #[test]
    fn counts_saturate() {
        let b = table();
        let t = Instant::now();
        let mut p = Pending::default();
        for _ in 0..8 {
            p.feed(&b, ch('9'), t, true);
        }
        assert_eq!(p.feed(&b, ch('j'), t, true), Outcome::Run { action: 1, count: Some(MAX_COUNT) });
    }

    #[test]
    fn a_wrong_key_ends_what_was_typed() {
        let b = table();
        let t = Instant::now();
        let mut p = Pending::default();
        p.feed(&b, ch('y'), t, true);
        assert_eq!(p.feed(&b, ch('x'), t, true), Outcome::Unbound { swallow: true });
        assert!(p.is_empty());
        // A Ctrl chord after a count goes on to the app's shortcuts.
        p.feed(&b, ch('4'), t, true);
        assert_eq!(p.feed(&b, Chord::new(Key::Char('k'), true, false, false), t, true), Outcome::Unbound { swallow: false });
        // Nothing pending: a stray key isn't swallowed.
        assert_eq!(p.feed(&b, ch('x'), t, true), Outcome::Unbound { swallow: false });
    }

    #[test]
    fn esc_cancels_only_what_is_pending() {
        let b = table();
        let t = Instant::now();
        let mut p = Pending::default();
        p.feed(&b, ch('5'), t, true);
        assert_eq!(p.feed(&b, Chord::plain(Key::Esc), t, true), Outcome::Cancelled);
        assert!(p.is_empty());
        assert_eq!(p.feed(&b, Chord::plain(Key::Esc), t, true), Outcome::Unbound { swallow: false });
    }

    #[test]
    fn timeout_runs_a_binding_that_is_also_a_prefix() {
        let b = table();
        let t = Instant::now();
        let mut p = Pending::default();
        p.feed(&b, ch('2'), t, true);
        p.feed(&b, ch('g'), t, true);
        assert_eq!(p.expire(&b, t + TIMEOUT / 2), None);
        assert_eq!(p.expire(&b, t + TIMEOUT), Some(Outcome::Run { action: 3, count: Some(2) }));
        assert!(p.is_empty());
        // Typed in time, the longer one wins.
        p.feed(&b, ch('g'), t, true);
        assert_eq!(p.feed(&b, ch('g'), t + TIMEOUT / 2, true), Outcome::Run { action: 2, count: None });
        // A mere start is dropped; a lone count never expires.
        p.feed(&b, ch('y'), t, true);
        assert_eq!(p.expire(&b, t + TIMEOUT), Some(Outcome::Cancelled));
        p.feed(&b, ch('7'), t, true);
        assert_eq!(p.expire(&b, t + TIMEOUT * 10), None);
        assert_eq!(p.display(), "7");
    }

    #[test]
    fn board_table_keeps_the_old_keys() {
        let b = board();
        let get = |s: &str| match b.lookup(&parse(s).unwrap()) {
            Lookup::Exact(a) => Some(a),
            _ => None,
        };
        assert_eq!(get("j"), Some(Action::Move(Dir::Down)));
        assert_eq!(get("J"), Some(Action::Pan(Dir::Down)));
        assert_eq!(get("<S-Down>"), Some(Action::Pan(Dir::Down)));
        assert_eq!(get("f"), Some(Action::LinkHints));
        assert_eq!(get("F"), Some(Action::Fit));
        assert_eq!(get("q"), Some(Action::Quit));
        assert_eq!(get("Q"), None, "Shift+Q never quit");
        assert_eq!(get("<C-S-z>"), Some(Action::Redo));
        // Keys the app keeps for itself, or that switch modes, aren't board bindings.
        for s in ["<Esc>", "<Tab>", "?", "<F1>", "<F2>", "<Menu>", "1", "0"] {
            assert_eq!(b.lookup(&parse(s).unwrap()), Lookup::None, "{s}");
        }
        // No board key waits for another.
        assert!(b.iter().all(|(seq, _)| seq.len() == 1));
    }

    #[test]
    fn page_table() {
        let b = page();
        let t = Instant::now();
        let mut p = Pending::default();
        let mut feed = |s: &str| {
            let mut out = Outcome::Wait;
            for c in parse(s).unwrap() {
                out = p.feed(&b, c, t, true);
            }
            out
        };
        assert_eq!(feed("5j"), Outcome::Run { action: Action::Scroll(Dir::Down), count: Some(5) });
        assert_eq!(feed("gg"), Outcome::Run { action: Action::Top, count: None });
        assert_eq!(feed("gi"), Outcome::Run { action: Action::FocusField, count: None });
        assert_eq!(feed("yy"), Outcome::Run { action: Action::CopyUrl, count: None });
        assert_eq!(feed("ym"), Outcome::Run { action: Action::CopyMarkdown, count: None });
        assert_eq!(feed("p"), Outcome::Run { action: Action::PasteHere, count: None });
        assert_eq!(feed("P"), Outcome::Run { action: Action::PasteBeside, count: None });
        assert_eq!(feed("O"), Outcome::Run { action: Action::AddressBarBeside, count: None });
        // A count doesn't repeat opening or copying.
        assert!(!Action::PasteHere.repeats() && !Action::CopyMarkdown.repeats());
        assert_eq!(feed("0"), Outcome::Run { action: Action::LeftEdge, count: None });
        assert_eq!(feed("$"), Outcome::Run { action: Action::RightEdge, count: None });
        assert_eq!(feed("f"), Outcome::Run { action: Action::LinkHintsHere, count: None });
        assert_eq!(feed("F"), Outcome::Run { action: Action::LinkHints, count: None });
        assert_eq!(feed("s"), Outcome::Run { action: Action::Sections, count: None });
        assert_eq!(feed("]s"), Outcome::Run { action: Action::NextSection, count: None });
        assert_eq!(feed("[s"), Outcome::Run { action: Action::PrevSection, count: None });
        assert_eq!(feed("S"), Outcome::Run { action: Action::OuterSection, count: None });
        // g waits; a key that isn't a g-sequence ends it, and is swallowed.
        assert_eq!(feed("g"), Outcome::Wait);
        assert_eq!(feed("x"), Outcome::Unbound { swallow: true });
        // Keys that switch modes stay out of the table.
        for s in ["<Esc>", "<S-Esc>", "<Tab>", "<Enter>", "<Space>", "<C-f>", "?"] {
            assert_eq!(b.lookup(&parse(s).unwrap()), Lookup::None, "{s}");
        }
    }
}
