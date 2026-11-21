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

use crate::hint;
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

    /// Binds a sequence already read; a later binding of it replaces the earlier one.
    pub fn bind_chords(&mut self, chords: Vec<Chord>, action: A) {
        for n in 1..chords.len() {
            self.prefixes.insert(chords[..n].to_vec());
        }
        self.map.insert(chords, action);
    }

    /// Drops the binding of `seq`; when there is none, every binding that
    /// starts with it (`;` drops all the hint kinds). How many went.
    pub fn unbind(&mut self, seq: &[Chord]) -> usize {
        let before = self.map.len();
        if self.map.remove(seq).is_none() {
            self.map.retain(|k, _| !k.starts_with(seq));
        }
        let gone = before - self.map.len();
        if gone > 0 {
            self.prefixes = self.map.keys().flat_map(|k| (1..k.len()).map(|n| k[..n].to_vec())).collect();
        }
        gone
    }

    /// Every binding, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (&[Chord], A)> {
        self.map.iter().map(|(k, a)| (k.as_slice(), *a))
    }

    /// The bindings longer than `prefix` that start with it, with `prefix`
    /// taken off (what can follow a half-typed sequence).
    pub fn after(&self, prefix: &[Chord]) -> Bindings<A> {
        let mut out = Bindings::new();
        for (k, a) in self.iter().filter(|(k, _)| k.len() > prefix.len() && k.starts_with(prefix)) {
            out.bind_chords(k[prefix.len()..].to_vec(), a);
        }
        out
    }

    /// Whether some sequence is bound to `action`.
    pub fn has(&self, action: A) -> bool
    where
        A: PartialEq,
    {
        self.map.values().any(|a| *a == action)
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

    /// When the last key of a half-typed sequence came.
    pub fn since(&self) -> Option<Instant> {
        self.at.filter(|_| !self.keys.is_empty())
    }

    /// The keys of a half-typed sequence (not its count).
    pub fn keys(&self) -> &[Chord] {
        &self.keys
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

    /// Ends a sequence that has waited [`TIMEOUT`] and is a binding by
    /// itself: it runs. A mere start keeps waiting for its next key (the
    /// panel of what can follow shows meanwhile; Esc drops it), as does a
    /// lone count. A start whose bindings went (keys reloaded) is dropped.
    pub fn expire<A: Copy>(&mut self, table: &Bindings<A>, now: Instant) -> Option<Outcome<A>> {
        let at = self.at?;
        if now.duration_since(at) < TIMEOUT {
            return None;
        }
        let out = match table.lookup(&self.keys) {
            Lookup::ExactAndPrefix(action) | Lookup::Exact(action) => Outcome::Run { action, count: self.count },
            Lookup::Prefix => return None,
            Lookup::None => Outcome::Cancelled,
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
    /// Link hints of one kind (`;` then a letter).
    Hint(hint::Kind),
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
    /// Copy part of the card's address or title (`yy`, `yt`, `ym`, `yd`,
    /// `yp`); to the primary selection when the flag is set (`yY`, `yT`…).
    Copy(Yank, bool),
    /// Open the clipboard's address (or search for its text) in this card / beside.
    PasteHere,
    PasteBeside,
    /// The same with the primary selection (`pp` / `PP`).
    PrimaryHere,
    PrimaryBeside,
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
    /// The palette as a command line (`:`), on the board and in a page.
    CommandLine,
    /// The last command line again (`.`); a count replaces its count.
    RepeatLine,
    // Cards from a page (M12 step 7); in Page mode the keys stay with the
    // page of the card moved to.
    /// The next / previous card in reading order (`J` / `K`), going round.
    NextCard,
    PrevCard,
    /// The first / last card in reading order (`g0` / `g$`).
    FirstCard,
    LastCard,
    /// The nth card in reading order (`Alt+1`…`Alt+8`); 9 is the last.
    NthCard(u8),
    /// The card used before this one (`Ctrl+^`).
    OtherCard,
    /// Pick a card by title (`gt`, `T`): the command line's `card` list.
    PickCard,
    /// Reopen the card closed last (`u`).
    Reopen,
    /// Close the other cards in the space, after asking (`co`).
    CloseOthers,
    /// Open the card's page again beside it (`gC`).
    Clone,
    // Jumps, marks and saved pages (M12 step 9).
    /// Back / forward through the cards visited (`Ctrl+o` / `Ctrl+i`).
    JumpBack,
    JumpForward,
    /// Set mark `c` (`m` then a letter in a page, `M` then a letter on
    /// the board): a lower-case letter the card's scroll spot, an
    /// upper-case one the board spot.
    SetMark(char),
    /// Go to mark `c` (`'` then a letter); `'` itself is the spot before
    /// the last jump (`''`).
    GoMark(char),
    /// Open a saved page here / beside (`b` / `B`): the command line's
    /// `saved` list.
    OpenSaved,
    OpenSavedBeside,
    /// A command line from the user's key file (M12 step 10): the app
    /// keeps the lines, this is the index of one.
    Line(u16),
    // Toggles under `t` (M12 step 11), on the board and in a page.
    /// Pages dark if they are light, else light (`td`).
    DarkPages,
    /// Show or hide the space panel at the top left (`th`).
    SpacePanel,
    /// Switch JavaScript, images, cookies or ads for the current card's
    /// site: saved (`tj`, `ti`, `tc`, `ta`), or until quitting when the
    /// flag is set (`tJ`, `tI`, `tC`, `tA`).
    SiteToggle(setcmd::SiteSwitch, bool),
    // Page navigation extras (M12 step 12).
    /// The page's next / previous page (`]]` / `[[`): its `rel` link, else
    /// a link that says so.
    NextPage,
    PrevPage,
    /// Up the address's path (`gu`, a count goes further) / to the site's
    /// root (`gU`).
    UrlUp,
    UrlRoot,
    /// The last number in the address up / down (`Ctrl+a` / `Ctrl+x`; a
    /// count is how far).
    Increment,
    Decrement,
    /// The address bar holding the card's address, to edit, opening here /
    /// beside (`go` / `gO`).
    EditAddress,
    EditAddressBeside,
    /// An empty address bar whose Enter opens beside and stays (`xo`).
    AddressBarBehind,
    /// Stop loading (`Ctrl+s`; when nothing loads, Ctrl+S saves as before).
    Stop,
    /// A start page beside the card (`gh`).
    Home,
    /// The page's source in a card beside (`gf`).
    ViewSource,
    /// Download the page itself (`gd`).
    DownloadPage,
    /// The window full screen or not (`F11`).
    Fullscreen,
    // Caret and Visual mode (M12 step 14).
    /// A caret in the page's text (`v`): at a `/` match or a selection in
    /// view, else the top of the view.
    Caret,
    /// Select whole lines (`V`, in a page and in Visual mode).
    VisualLine,
    /// Select by characters from the caret, or stop (`v` in Visual mode).
    SelectChars,
    /// Move the caret, or the moving end of the selection.
    CaretMove(caret::Motion),
    /// The selection's other end moves now (`o`).
    SwapEnds,
    /// Copy the selection and leave (`y`, Enter); to the primary selection
    /// when the flag is set (`Y`).
    CopySelection(bool),
    /// Esc: the selection goes, then the caret.
    CaretBack,
    // Typing in a page (M12 step 15).
    /// The field's text in your editor; saved, it comes back (`Ctrl+e`).
    EditInEditor,
    /// Paste the primary selection into the field (`Shift+Insert`).
    PastePrimary,
}

/// What a copy key copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Yank {
    Url,
    Title,
    /// `[title](url)`.
    Markdown,
    /// The site's host (`news.example.com`).
    Site,
    /// The address without tracking parts (`scene::address::clean`).
    Clean,
}

use Action as Ac;
use crate::caret::{self, Motion as Mo};
use crate::setcmd::{self, SiteSwitch as SW};
use crate::sitesettings::Content;

/// The actions a key file can name, in the order the `?` sheet lists them,
/// with what each does. Marks and Alt+digits are reached by command lines
/// (`mark a`, `to-mark a`, `card 3`).
pub const NAMED: &[(&str, Action, &str)] = &[
    ("move-left", Ac::Move(Dir::Left), "Go to the card on the left"),
    ("move-down", Ac::Move(Dir::Down), "Go to the card below"),
    ("move-up", Ac::Move(Dir::Up), "Go to the card above"),
    ("move-right", Ac::Move(Dir::Right), "Go to the card on the right"),
    ("enter", Ac::Enter, "Type in the card's page"),
    ("scroll-down", Ac::Scroll(Dir::Down), "Scroll down"),
    ("scroll-up", Ac::Scroll(Dir::Up), "Scroll up"),
    ("scroll-left", Ac::Scroll(Dir::Left), "Scroll left"),
    ("scroll-right", Ac::Scroll(Dir::Right), "Scroll right"),
    ("half-down", Ac::ScrollDown, "Scroll half a section down"),
    ("half-up", Ac::ScrollUp, "Scroll half a section up"),
    ("top", Ac::Top, "To the top"),
    ("bottom", Ac::Bottom, "To the bottom"),
    ("left-edge", Ac::LeftEdge, "To the left edge"),
    ("right-edge", Ac::RightEdge, "To the right edge"),
    ("sections", Ac::Sections, "Label the scroll areas; type one to scroll it"),
    ("next-section", Ac::NextSection, "Next scroll area"),
    ("prev-section", Ac::PrevSection, "Previous scroll area"),
    ("outer-section", Ac::OuterSection, "The scroll area around this one"),
    ("hints-here", Ac::LinkHintsHere, "Follow a link here"),
    ("hints", Ac::LinkHints, "Label links and controls; type one to use it (beside)"),
    ("hint-yank", Ac::Hint(hint::Kind::Yank), "Hints: copy a link's address"),
    ("hint-download", Ac::Hint(hint::Kind::Download), "Hints: download a link or image"),
    ("hint-image", Ac::Hint(hint::Kind::Image), "Hints: open an image beside"),
    ("hint-beside", Ac::Hint(hint::Kind::Background), "Hints: open beside, stay here"),
    ("hint-field", Ac::Hint(hint::Kind::Field), "Hints: type in a field"),
    ("hint-many", Ac::Hint(hint::Kind::Many), "Hints: open many beside, until Esc"),
    ("hint-edit", Ac::Hint(hint::Kind::Edit), "Hints: a link's address to edit in :open"),
    ("hint-hover", Ac::Hint(hint::Kind::Hover), "Hints: hover over something"),
    ("hint-click", Ac::Hint(hint::Kind::Click), "Hints: click anything clickable"),
    ("hint-image-here", Ac::Hint(hint::Kind::ImageHere), "Hints: open an image here"),
    ("hint-primary", Ac::Hint(hint::Kind::Primary), "Hints: a link's address to the primary selection"),
    ("back", Ac::Back, "Back"),
    ("forward", Ac::Forward, "Forward"),
    ("reload", Ac::Reload, "Reload"),
    ("reload-fresh", Ac::ReloadFresh, "Reload, skipping the cache"),
    ("insert", Ac::Insert, "Type in the page (Esc ends)"),
    ("focus-field", Ac::FocusField, "Type in the page's first field"),
    ("passthrough", Ac::Passthrough, "All keys to the page (Shift+Esc ends)"),
    ("copy-url", Ac::Copy(Yank::Url, false), "Copy the address"),
    ("copy-title", Ac::Copy(Yank::Title, false), "Copy the title"),
    ("copy-markdown", Ac::Copy(Yank::Markdown, false), "Copy a Markdown link"),
    ("copy-site", Ac::Copy(Yank::Site, false), "Copy the site"),
    ("copy-clean", Ac::Copy(Yank::Clean, false), "Copy the address without tracking parts"),
    ("primary-url", Ac::Copy(Yank::Url, true), "The address to the primary selection"),
    ("primary-title", Ac::Copy(Yank::Title, true), "The title to the primary selection"),
    ("primary-markdown", Ac::Copy(Yank::Markdown, true), "A Markdown link to the primary selection"),
    ("primary-site", Ac::Copy(Yank::Site, true), "The site to the primary selection"),
    ("primary-clean", Ac::Copy(Yank::Clean, true), "The clean address to the primary selection"),
    ("paste-here", Ac::PasteHere, "Open the copied address here"),
    ("paste-beside", Ac::PasteBeside, "Open the copied address beside"),
    ("primary-here", Ac::PrimaryHere, "Open the primary selection here"),
    ("primary-beside", Ac::PrimaryBeside, "Open the primary selection beside"),
    ("address-bar", Ac::AddressBar, "Address bar"),
    ("address-bar-beside", Ac::AddressBarBeside, "Address bar, opening beside"),
    ("address-bar-behind", Ac::AddressBarBehind, "Address bar, opening beside and staying here"),
    ("edit-address", Ac::EditAddress, "Edit the address"),
    ("edit-address-beside", Ac::EditAddressBeside, "Edit the address, opening beside"),
    ("next-page", Ac::NextPage, "The page's next page"),
    ("prev-page", Ac::PrevPage, "The page's previous page"),
    ("url-up", Ac::UrlUp, "Up the address's path"),
    ("url-root", Ac::UrlRoot, "To the site's root"),
    ("increment", Ac::Increment, "The address's last number up"),
    ("decrement", Ac::Decrement, "The address's last number down"),
    ("stop", Ac::Stop, "Stop loading"),
    ("home", Ac::Home, "A start page beside the card"),
    ("view-source", Ac::ViewSource, "The page's source, beside"),
    ("download-page", Ac::DownloadPage, "Download the page"),
    ("fullscreen", Ac::Fullscreen, "Full screen on or off"),
    ("caret", Ac::Caret, "A caret in the text, to select with (v)"),
    ("visual-line", Ac::VisualLine, "Select whole lines"),
    ("select-chars", Ac::SelectChars, "Select from the caret, or stop"),
    ("caret-left", Ac::CaretMove(Mo::Left), "Caret: a character left"),
    ("caret-right", Ac::CaretMove(Mo::Right), "Caret: a character right"),
    ("caret-down", Ac::CaretMove(Mo::Down), "Caret: a line down"),
    ("caret-up", Ac::CaretMove(Mo::Up), "Caret: a line up"),
    ("caret-word", Ac::CaretMove(Mo::WordNext), "Caret: the next word"),
    ("caret-word-back", Ac::CaretMove(Mo::WordBack), "Caret: back a word"),
    ("caret-word-end", Ac::CaretMove(Mo::WordEnd), "Caret: the end of the word"),
    ("caret-line-start", Ac::CaretMove(Mo::LineStart), "Caret: the start of the line"),
    ("caret-line-end", Ac::CaretMove(Mo::LineEnd), "Caret: the end of the line"),
    ("caret-top", Ac::CaretMove(Mo::Top), "Caret: the start of the page"),
    ("caret-bottom", Ac::CaretMove(Mo::Bottom), "Caret: the end of the page"),
    ("caret-para-back", Ac::CaretMove(Mo::ParaBack), "Caret: back a paragraph"),
    ("caret-para", Ac::CaretMove(Mo::ParaNext), "Caret: the next paragraph"),
    ("swap-ends", Ac::SwapEnds, "Move the selection's other end"),
    ("copy-selection", Ac::CopySelection(false), "Copy the selection"),
    ("primary-selection", Ac::CopySelection(true), "The selection to the primary selection"),
    ("caret-back", Ac::CaretBack, "Drop the selection, then the caret"),
    ("find", Ac::Find, "Find in the page"),
    ("find-next", Ac::FindNext, "Next match"),
    ("find-prev", Ac::FindPrev, "Previous match"),
    ("next-card", Ac::NextCard, "Next card (reading order)"),
    ("prev-card", Ac::PrevCard, "Previous card (reading order)"),
    ("pick-card", Ac::PickCard, "Go to a card by title"),
    ("first-card", Ac::FirstCard, "First card"),
    ("last-card", Ac::LastCard, "Last card"),
    ("other-card", Ac::OtherCard, "The card used before this one"),
    ("jump-back", Ac::JumpBack, "Back through the cards visited"),
    ("jump-forward", Ac::JumpForward, "Forward through the cards visited"),
    ("open-saved", Ac::OpenSaved, "Open a saved page here"),
    ("open-saved-beside", Ac::OpenSavedBeside, "Open a saved page beside"),
    ("close", Ac::Close, "Close the card (or the selection)"),
    ("reopen", Ac::Reopen, "Reopen the last closed card"),
    ("close-others", Ac::CloseOthers, "Close the other cards (asks first)"),
    ("clone", Ac::Clone, "Open this page again beside it"),
    ("command-line", Ac::CommandLine, "Command line"),
    ("edit-in-editor", Ac::EditInEditor, "Edit the field's text in your editor"),
    ("paste-primary", Ac::PastePrimary, "Paste the primary selection"),
    ("repeat-line", Ac::RepeatLine, "The last command line again"),
    ("pan-left", Ac::Pan(Dir::Left), "Pan left"),
    ("pan-down", Ac::Pan(Dir::Down), "Pan down"),
    ("pan-up", Ac::Pan(Dir::Up), "Pan up"),
    ("pan-right", Ac::Pan(Dir::Right), "Pan right"),
    ("zoom-in", Ac::ZoomIn, "Zoom in"),
    ("zoom-out", Ac::ZoomOut, "Zoom out"),
    ("fit", Ac::Fit, "Fit everything in view"),
    ("fill", Ac::Fill, "Fill the view with the card"),
    ("select", Ac::Select, "Select / deselect the card"),
    ("select-all", Ac::SelectAll, "Select every card"),
    ("arrange", Ac::Arrange, "Move the card (then h j k l; Shift resizes)"),
    ("note", Ac::NewNote, "New note"),
    ("sticky", Ac::NewSticky, "New sticky note"),
    ("tidy", Ac::Tidy, "Tidy trails"),
    ("fold", Ac::Fold, "Fold / unfold the trail"),
    ("outline", Ac::Outline, "List a stack's pages"),
    ("stack-prev", Ac::StackPrev, "Peek back through a stack"),
    ("stack-next", Ac::StackNext, "Peek on through a stack"),
    ("name-trail", Ac::NameTrail, "Name the trail"),
    ("trail-colour", Ac::TrailColour, "Next trail colour"),
    ("sidebar", Ac::Sidebar, "Trails sidebar"),
    ("dark-pages", Ac::DarkPages, "Pages dark or light"),
    ("space-panel", Ac::SpacePanel, "Show or hide the space panel"),
    ("site-javascript", Ac::SiteToggle(SW::Content(Content::JavaScript), false), "JavaScript on or off for this site"),
    ("site-images", Ac::SiteToggle(SW::Content(Content::Images), false), "Images on or off for this site"),
    ("site-cookies", Ac::SiteToggle(SW::Content(Content::Cookies), false), "Cookies on or off for this site"),
    ("site-ads", Ac::SiteToggle(SW::Ads, false), "Ads allowed or blocked on this site"),
    ("session-javascript", Ac::SiteToggle(SW::Content(Content::JavaScript), true), "JavaScript for this site, until you quit"),
    ("session-images", Ac::SiteToggle(SW::Content(Content::Images), true), "Images for this site, until you quit"),
    ("session-cookies", Ac::SiteToggle(SW::Content(Content::Cookies), true), "Cookies for this site, until you quit"),
    ("session-ads", Ac::SiteToggle(SW::Ads, true), "Ads on this site, until you quit"),
    ("site-passthrough", Ac::SiteToggle(SW::Content(Content::Passthrough), false), "This site starts in Passthrough"),
    ("undo", Ac::Undo, "Undo"),
    ("redo", Ac::Redo, "Redo"),
    ("quit", Ac::Quit, "Quit Trailboard"),
];

impl Action {
    /// The action called `name` in a key file (exactly, case ignored).
    pub fn named(name: &str) -> Option<Action> {
        NAMED.iter().find(|(n, ..)| n.eq_ignore_ascii_case(name)).map(|(_, a, _)| *a)
    }

    /// Its name in a key file; marks, card numbers and lines have none.
    pub fn name(self) -> Option<&'static str> {
        NAMED.iter().find(|(_, a, _)| *a == self).map(|(n, ..)| *n)
    }

    /// What it does, as the `?` sheet says it (a line says itself elsewhere).
    pub fn about(self) -> String {
        match self {
            Ac::SetMark(c) if c.is_ascii_lowercase() => "Mark the page's scroll spot".into(),
            Ac::SetMark(_) => "Mark the board spot".into(),
            Ac::GoMark('\'') => "Back to before the last jump".into(),
            Ac::GoMark(c) if c.is_ascii_lowercase() => "Go to a page mark".into(),
            Ac::GoMark(_) => "Go to a board mark".into(),
            Ac::NthCard(9) => "The last card".into(),
            Ac::NthCard(_) => "The nth card in reading order".into(),
            Ac::Line(n) => format!("Line {n}"),
            a => NAMED.iter().find(|(_, x, _)| *x == a).map_or_else(String::new, |(.., about)| about.to_string()),
        }
    }

    /// Where the `?` sheet lists it: its place in [`NAMED`]; marks and card
    /// numbers after the jumps, lines last.
    pub fn rank(self) -> usize {
        let after_jumps = NAMED.iter().position(|(n, ..)| *n == "jump-forward").unwrap_or(0) * 4 + 1;
        match self {
            Ac::NthCard(_) => NAMED.iter().position(|(n, ..)| *n == "other-card").unwrap_or(0) * 4 + 1,
            Ac::SetMark(c) if c.is_ascii_lowercase() => after_jumps,
            Ac::GoMark(c) if c.is_ascii_lowercase() => after_jumps + 1,
            Ac::SetMark(_) | Ac::GoMark(_) => after_jumps + 2,
            Ac::Line(n) => NAMED.len() * 4 + n as usize,
            a => NAMED.iter().position(|(_, x, _)| *x == a).map_or(usize::MAX, |i| i * 4),
        }
    }
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
                | Action::NextCard
                | Action::PrevCard
                | Action::Reopen
                | Action::JumpBack
                | Action::JumpForward
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
        // Owner (2026-11-20): t starts toggles (`bind_toggles`); Tidy is T.
        ("T", Tidy),
        ("m", Arrange),
        ("n", NewNote),
        ("N", NewSticky),
        // Owner (2026-11-20): b / B open saved pages and the sidebar is
        // Ctrl+b (`bind_jumps`).
        ("v", Select),
        ("<C-a>", SelectAll),
        ("z", Fill),
        // Owner (2026-11-20): d closes and u reopens, as in a page; half a
        // page scrolls with Ctrl+d / Ctrl+u.
        ("d", Close),
        ("u", Reopen),
        ("<C-d>", ScrollDown),
        ("<C-u>", ScrollUp),
        ("<C-^>", OtherCard),
        ("<C-6>", OtherCard),
        ("<C-S-6>", OtherCard),
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
        (":", CommandLine),
        (".", RepeatLine),
        ("<F11>", Fullscreen),
    ];
    for (seq, a) in table {
        b.bind(seq, *a).expect("board keys parse");
    }
    bind_hint_kinds(&mut b);
    bind_card_numbers(&mut b);
    // m moves the card on the board (then h/j/k/l), so marks are set with M.
    bind_marks(&mut b, "M");
    bind_jumps(&mut b);
    bind_toggles(&mut b);
    b
}

/// `t` then a letter, on the board and in a page: d dark pages, h the
/// space panel (its header), s the sidebar; j / i / c / a JavaScript,
/// images, cookies and ads for the current card's site, saved; the same
/// letters with Shift for this session only; p the site starts in
/// Passthrough (saved).
fn bind_toggles(b: &mut Bindings<Action>) {
    b.bind("td", Action::DarkPages).expect("toggle keys parse");
    b.bind("th", Action::SpacePanel).expect("toggle keys parse");
    b.bind("ts", Action::Sidebar).expect("toggle keys parse");
    b.bind("tp", Action::SiteToggle(SW::Content(Content::Passthrough), false)).expect("toggle keys parse");
    for (c, sw) in [('j', SW::Content(Content::JavaScript)), ('i', SW::Content(Content::Images)), ('c', SW::Content(Content::Cookies)), ('a', SW::Ads)] {
        b.bind(&format!("t{c}"), Action::SiteToggle(sw, false)).expect("toggle keys parse");
        b.bind(&format!("t{}", c.to_ascii_uppercase()), Action::SiteToggle(sw, true)).expect("toggle keys parse");
    }
}

/// The mark letters: `a`–`z` (a card's scroll spot) and `A`–`Z` (a board spot).
pub fn mark_letters() -> impl Iterator<Item = char> {
    ('a'..='z').chain('A'..='Z')
}

/// `{set}` then a letter sets a mark; `'` or `` ` `` then a letter goes to
/// one, and `''` (or ``` `` ```) to the spot before the last jump.
fn bind_marks(b: &mut Bindings<Action>, set: &str) {
    for c in mark_letters() {
        b.bind(&format!("{set}{c}"), Action::SetMark(c)).expect("mark keys parse");
        b.bind(&format!("'{c}"), Action::GoMark(c)).expect("mark keys parse");
        b.bind(&format!("`{c}"), Action::GoMark(c)).expect("mark keys parse");
    }
    b.bind("''", Action::GoMark('\'')).expect("mark keys parse");
    b.bind("``", Action::GoMark('\'')).expect("mark keys parse");
}

/// Ctrl+o / Ctrl+i: back and forward through the cards visited; b / B
/// saved pages here / beside; Ctrl+b the trails sidebar.
fn bind_jumps(b: &mut Bindings<Action>) {
    for (seq, a) in [
        ("<C-o>", Action::JumpBack),
        ("<C-i>", Action::JumpForward),
        ("b", Action::OpenSaved),
        ("B", Action::OpenSavedBeside),
        ("<C-b>", Action::Sidebar),
    ] {
        b.bind(seq, a).expect("jump keys parse");
    }
}

/// `Alt+1`…`Alt+9`: the nth card in reading order (9: the last).
fn bind_card_numbers(b: &mut Bindings<Action>) {
    for n in 1..=9u8 {
        b.bind(&format!("<A-{n}>"), Action::NthCard(n)).expect("card number keys parse");
    }
}

/// `;` then a letter: link hints of that kind, on the board and in a page.
fn bind_hint_kinds(b: &mut Bindings<Action>) {
    for k in hint::Kind::ALL {
        b.bind(&format!(";{}", k.key()), Action::Hint(k)).expect("hint keys parse");
    }
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
        // Owner (2026-11-20): d closes the card, u reopens; Ctrl+d / Ctrl+u
        // scroll half a section.
        ("d", Close),
        ("u", Reopen),
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
        ("yy", Copy(Yank::Url, false)),
        ("yt", Copy(Yank::Title, false)),
        ("ym", Copy(Yank::Markdown, false)),
        ("yd", Copy(Yank::Site, false)),
        ("yp", Copy(Yank::Clean, false)),
        // Shift: to the primary selection.
        ("yY", Copy(Yank::Url, true)),
        ("yT", Copy(Yank::Title, true)),
        ("yM", Copy(Yank::Markdown, true)),
        ("yD", Copy(Yank::Site, true)),
        ("yP", Copy(Yank::Clean, true)),
        // p alone waits a moment for a second p (the primary selection).
        ("p", PasteHere),
        ("P", PasteBeside),
        ("pp", PrimaryHere),
        ("PP", PrimaryBeside),
        ("o", AddressBar),
        ("O", AddressBarBeside),
        ("go", EditAddress),
        ("gO", EditAddressBeside),
        ("xo", AddressBarBehind),
        // Moving about by address and by the page's own links.
        ("]]", NextPage),
        ("[[", PrevPage),
        ("gu", UrlUp),
        ("gU", UrlRoot),
        ("<C-a>", Increment),
        ("<C-x>", Decrement),
        ("<C-s>", Stop),
        ("gh", Home),
        ("gf", ViewSource),
        ("gd", DownloadPage),
        ("<F11>", Fullscreen),
        ("/", Find),
        ("n", FindNext),
        ("N", FindPrev),
        ("<C-v>", Passthrough),
        ("v", Caret),
        ("V", VisualLine),
        ("s", Sections),
        ("]s", NextSection),
        ("[s", PrevSection),
        ("S", OuterSection),
        (":", CommandLine),
        (".", RepeatLine),
        // Cards, as tabs are elsewhere.
        ("J", NextCard),
        ("K", PrevCard),
        ("gt", PickCard),
        ("T", PickCard),
        ("g0", FirstCard),
        ("g$", LastCard),
        ("<C-^>", OtherCard),
        ("<C-6>", OtherCard),
        ("<C-S-6>", OtherCard),
        ("co", CloseOthers),
        ("gC", Clone),
        // Ctrl+w then a direction: the nearest card that way. Ctrl+w alone
        // still closes the card, once the pause shows nothing follows.
        ("<C-w>", Close),
        ("<C-w>c", Close),
        ("<C-w>q", Close),
        ("<C-w>h", Move(Dir::Left)),
        ("<C-w>j", Move(Dir::Down)),
        ("<C-w>k", Move(Dir::Up)),
        ("<C-w>l", Move(Dir::Right)),
        ("<C-w><Left>", Move(Dir::Left)),
        ("<C-w><Down>", Move(Dir::Down)),
        ("<C-w><Up>", Move(Dir::Up)),
        ("<C-w><Right>", Move(Dir::Right)),
        ("<C-w>w", NextCard),
        ("<C-w>W", PrevCard),
        ("<C-w>p", OtherCard),
    ];
    for (seq, a) in table {
        b.bind(seq, *a).expect("page keys parse");
    }
    bind_hint_kinds(&mut b);
    bind_card_numbers(&mut b);
    bind_marks(&mut b, "m");
    bind_jumps(&mut b);
    bind_toggles(&mut b);
    b
}

/// Caret and Visual mode's keys (M12 step 14). Keys not here do nothing
/// in the page; Ctrl and Alt shortcuts the app has still work.
pub fn visual() -> Bindings<Action> {
    use Action::*;
    let mut b = Bindings::new();
    let table: &[(&str, Action)] = &[
        ("h", CaretMove(Mo::Left)),
        ("l", CaretMove(Mo::Right)),
        ("j", CaretMove(Mo::Down)),
        ("k", CaretMove(Mo::Up)),
        ("<Left>", CaretMove(Mo::Left)),
        ("<Right>", CaretMove(Mo::Right)),
        ("<Down>", CaretMove(Mo::Down)),
        ("<Up>", CaretMove(Mo::Up)),
        ("w", CaretMove(Mo::WordNext)),
        ("W", CaretMove(Mo::WordNext)),
        ("b", CaretMove(Mo::WordBack)),
        ("B", CaretMove(Mo::WordBack)),
        ("e", CaretMove(Mo::WordEnd)),
        ("E", CaretMove(Mo::WordEnd)),
        ("0", CaretMove(Mo::LineStart)),
        ("^", CaretMove(Mo::LineStart)),
        ("<Home>", CaretMove(Mo::LineStart)),
        ("$", CaretMove(Mo::LineEnd)),
        ("<End>", CaretMove(Mo::LineEnd)),
        ("gg", CaretMove(Mo::Top)),
        ("G", CaretMove(Mo::Bottom)),
        ("{", CaretMove(Mo::ParaBack)),
        ("}", CaretMove(Mo::ParaNext)),
        ("v", SelectChars),
        ("V", VisualLine),
        ("o", SwapEnds),
        ("y", CopySelection(false)),
        ("<Enter>", CopySelection(false)),
        ("Y", CopySelection(true)),
        ("<Esc>", CaretBack),
        (":", CommandLine),
        ("<F11>", Fullscreen),
    ];
    for (seq, a) in table {
        b.bind(seq, *a).expect("visual keys parse");
    }
    b
}

/// Keys while typing in a page (Insert mode, M12 step 15). Every other
/// key goes to the page.
pub fn insert() -> Bindings<Action> {
    let mut b = Bindings::new();
    b.bind("<C-e>", Action::EditInEditor).expect("insert keys parse");
    b.bind("<S-Insert>", Action::PastePrimary).expect("insert keys parse");
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_keys_edit_and_paste() {
        let i = insert();
        assert_eq!(i.lookup(&parse("<C-e>").unwrap()), Lookup::Exact(Action::EditInEditor));
        assert_eq!(i.lookup(&parse("<S-Insert>").unwrap()), Lookup::Exact(Action::PastePrimary));
        for (_, a) in i.iter() {
            assert!(a.name().is_some(), "{a:?}");
        }
    }

    #[test]
    fn visual_keys_move_select_and_copy() {
        let v = visual();
        let get = |s: &str| match v.lookup(&parse(s).unwrap()) {
            Lookup::Exact(a) | Lookup::ExactAndPrefix(a) => Some(a),
            _ => None,
        };
        assert_eq!(get("w"), Some(Action::CaretMove(caret::Motion::WordNext)));
        assert_eq!(get("gg"), Some(Action::CaretMove(caret::Motion::Top)));
        assert_eq!(get("}"), Some(Action::CaretMove(caret::Motion::ParaNext)));
        assert_eq!(get("Y"), Some(Action::CopySelection(true)));
        assert_eq!(get("<Enter>"), Some(Action::CopySelection(false)));
        assert_eq!(get("<Esc>"), Some(Action::CaretBack));
        assert_eq!(get("d"), None);
        // v and V start the modes from a page.
        assert!(page().has(Action::Caret) && page().has(Action::VisualLine));
        // Every visual action has a name for the key file.
        for (_, a) in v.iter() {
            assert!(a.name().is_some(), "{a:?}");
        }
    }

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
        // A mere start keeps waiting, as does a lone count.
        p.feed(&b, ch('y'), t, true);
        assert_eq!(p.expire(&b, t + TIMEOUT * 10), None);
        assert_eq!(p.feed(&b, ch('y'), t + TIMEOUT * 10, true), Outcome::Run { action: 4, count: None });
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
        // d / u close and reopen; Ctrl+d / Ctrl+u scroll.
        assert_eq!(get("d"), Some(Action::Close));
        assert_eq!(get("u"), Some(Action::Reopen));
        assert_eq!(get("<C-d>"), Some(Action::ScrollDown));
        assert_eq!(get("<C-u>"), Some(Action::ScrollUp));
        assert_eq!(get("<A-3>"), Some(Action::NthCard(3)));
        assert_eq!(get("<C-S-6>"), Some(Action::OtherCard));
        // Only the hint kinds, marks and toggles wait for another key; m stays Arrange.
        assert!(b.iter().all(|(seq, a)| seq.len() == 1
            || matches!(a, Action::Hint(_) | Action::SetMark(_) | Action::GoMark(_))
            || seq[0] == ch('t')));
        // Owner (2026-11-20): t starts toggles everywhere, Tidy is T.
        assert_eq!(b.lookup(&parse("t").unwrap()), Lookup::Prefix);
        assert_eq!(get("T"), Some(Action::Tidy));
        assert_eq!(get("td"), Some(Action::DarkPages));
        assert_eq!(get("tj"), Some(Action::SiteToggle(SW::Content(Content::JavaScript), false)));
        assert_eq!(get("tA"), Some(Action::SiteToggle(SW::Ads, true)));
        assert_eq!(page().lookup(&parse("tc").unwrap()), Lookup::Exact(Action::SiteToggle(SW::Content(Content::Cookies), false)));
        assert_eq!(page().lookup(&parse("ts").unwrap()), Lookup::Exact(Action::Sidebar));
        assert_eq!(get("m"), Some(Action::Arrange));
        assert_eq!(get("Ma"), Some(Action::SetMark('a')));
        assert_eq!(get("'B"), Some(Action::GoMark('B')));
        assert_eq!(get("''"), Some(Action::GoMark('\'')));
        assert_eq!(get("b"), Some(Action::OpenSaved));
        assert_eq!(get("B"), Some(Action::OpenSavedBeside));
        assert_eq!(get("<C-b>"), Some(Action::Sidebar));
        assert_eq!(get("<C-o>"), Some(Action::JumpBack));
        assert_eq!(get("<C-i>"), Some(Action::JumpForward));
        assert_eq!(get(";y"), Some(Action::Hint(hint::Kind::Yank)));
        assert_eq!(get(";m"), Some(Action::Hint(hint::Kind::Many)));
    }

    #[test]
    fn p_waits_for_a_second_p() {
        let b = page();
        let t = Instant::now();
        let mut p = Pending::default();
        for (key, alone) in [('p', Action::PasteHere), ('P', Action::PasteBeside)] {
            assert_eq!(p.feed(&b, ch(key), t, true), Outcome::Wait);
            assert_eq!(p.expire(&b, t + TIMEOUT), Some(Outcome::Run { action: alone, count: None }));
        }
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
        assert_eq!(feed("yy"), Outcome::Run { action: Action::Copy(Yank::Url, false), count: None });
        assert_eq!(feed("ym"), Outcome::Run { action: Action::Copy(Yank::Markdown, false), count: None });
        assert_eq!(feed("O"), Outcome::Run { action: Action::AddressBarBeside, count: None });
        // A count doesn't repeat opening or copying.
        assert!(!Action::PasteHere.repeats() && !Action::Copy(Yank::Markdown, false).repeats());
        assert_eq!(feed("0"), Outcome::Run { action: Action::LeftEdge, count: None });
        assert_eq!(feed("$"), Outcome::Run { action: Action::RightEdge, count: None });
        assert_eq!(feed("f"), Outcome::Run { action: Action::LinkHintsHere, count: None });
        assert_eq!(feed("F"), Outcome::Run { action: Action::LinkHints, count: None });
        assert_eq!(feed("s"), Outcome::Run { action: Action::Sections, count: None });
        assert_eq!(feed("]s"), Outcome::Run { action: Action::NextSection, count: None });
        assert_eq!(feed("[s"), Outcome::Run { action: Action::PrevSection, count: None });
        assert_eq!(feed("S"), Outcome::Run { action: Action::OuterSection, count: None });
        for k in hint::Kind::ALL {
            assert_eq!(feed(&format!(";{}", k.key())), Outcome::Run { action: Action::Hint(k), count: None });
        }
        assert_eq!(feed(";"), Outcome::Wait);
        assert_eq!(feed("<Esc>"), Outcome::Cancelled);
        // g waits; a key that isn't a g-sequence ends it, and is swallowed.
        assert_eq!(feed("g"), Outcome::Wait);
        assert_eq!(feed("x"), Outcome::Unbound { swallow: true });
        // Cards from a page.
        assert_eq!(feed("d"), Outcome::Run { action: Action::Close, count: None });
        assert_eq!(feed("2u"), Outcome::Run { action: Action::Reopen, count: Some(2) });
        assert_eq!(feed("<C-d>"), Outcome::Run { action: Action::ScrollDown, count: None });
        assert_eq!(feed("3J"), Outcome::Run { action: Action::NextCard, count: Some(3) });
        assert_eq!(feed("K"), Outcome::Run { action: Action::PrevCard, count: None });
        assert_eq!(feed("gt"), Outcome::Run { action: Action::PickCard, count: None });
        assert_eq!(feed("T"), Outcome::Run { action: Action::PickCard, count: None });
        assert_eq!(feed("g0"), Outcome::Run { action: Action::FirstCard, count: None });
        assert_eq!(feed("g$"), Outcome::Run { action: Action::LastCard, count: None });
        assert_eq!(feed("<A-9>"), Outcome::Run { action: Action::NthCard(9), count: None });
        assert_eq!(feed("<C-^>"), Outcome::Run { action: Action::OtherCard, count: None });
        assert_eq!(feed("co"), Outcome::Run { action: Action::CloseOthers, count: None });
        assert_eq!(feed("gC"), Outcome::Run { action: Action::Clone, count: None });
        assert_eq!(feed("<C-w>l"), Outcome::Run { action: Action::Move(Dir::Right), count: None });
        assert_eq!(feed("<C-w>p"), Outcome::Run { action: Action::OtherCard, count: None });
        // Jumps, marks and saved pages.
        assert_eq!(feed("2<C-o>"), Outcome::Run { action: Action::JumpBack, count: Some(2) });
        assert_eq!(feed("<C-i>"), Outcome::Run { action: Action::JumpForward, count: None });
        assert_eq!(feed("ma"), Outcome::Run { action: Action::SetMark('a'), count: None });
        assert_eq!(feed("mZ"), Outcome::Run { action: Action::SetMark('Z'), count: None });
        assert_eq!(feed("'a"), Outcome::Run { action: Action::GoMark('a'), count: None });
        assert_eq!(feed("`Z"), Outcome::Run { action: Action::GoMark('Z'), count: None });
        assert_eq!(feed("''"), Outcome::Run { action: Action::GoMark('\''), count: None });
        assert_eq!(feed("m"), Outcome::Wait);
        assert_eq!(feed("1"), Outcome::Unbound { swallow: true }, "not a mark letter");
        assert_eq!(feed("b"), Outcome::Run { action: Action::OpenSaved, count: None });
        assert_eq!(feed("B"), Outcome::Run { action: Action::OpenSavedBeside, count: None });
        assert_eq!(feed("<C-b>"), Outcome::Run { action: Action::Sidebar, count: None });
        // Navigation extras (step 12).
        assert_eq!(feed("]]"), Outcome::Run { action: Action::NextPage, count: None });
        assert_eq!(feed("[["), Outcome::Run { action: Action::PrevPage, count: None });
        assert_eq!(feed("3gu"), Outcome::Run { action: Action::UrlUp, count: Some(3) });
        assert_eq!(feed("gU"), Outcome::Run { action: Action::UrlRoot, count: None });
        assert_eq!(feed("5<C-a>"), Outcome::Run { action: Action::Increment, count: Some(5) });
        assert_eq!(feed("<C-x>"), Outcome::Run { action: Action::Decrement, count: None });
        assert_eq!(feed("go"), Outcome::Run { action: Action::EditAddress, count: None });
        assert_eq!(feed("gO"), Outcome::Run { action: Action::EditAddressBeside, count: None });
        assert_eq!(feed("xo"), Outcome::Run { action: Action::AddressBarBehind, count: None });
        assert_eq!(feed("<C-s>"), Outcome::Run { action: Action::Stop, count: None });
        assert_eq!(feed("gh"), Outcome::Run { action: Action::Home, count: None });
        assert_eq!(feed("gf"), Outcome::Run { action: Action::ViewSource, count: None });
        assert_eq!(feed("gd"), Outcome::Run { action: Action::DownloadPage, count: None });
        assert_eq!(feed("<F11>"), Outcome::Run { action: Action::Fullscreen, count: None });
        assert_eq!(feed("yd"), Outcome::Run { action: Action::Copy(Yank::Site, false), count: None });
        assert_eq!(feed("yp"), Outcome::Run { action: Action::Copy(Yank::Clean, false), count: None });
        assert_eq!(feed("yY"), Outcome::Run { action: Action::Copy(Yank::Url, true), count: None });
        assert_eq!(feed("yD"), Outcome::Run { action: Action::Copy(Yank::Site, true), count: None });
        assert_eq!(feed("pp"), Outcome::Run { action: Action::PrimaryHere, count: None });
        assert_eq!(feed("PP"), Outcome::Run { action: Action::PrimaryBeside, count: None });
        assert_eq!(feed(";Y"), Outcome::Run { action: Action::Hint(hint::Kind::Primary), count: None });
        assert_eq!(feed(";I"), Outcome::Run { action: Action::Hint(hint::Kind::ImageHere), count: None });
        // Every action a key reaches has a name for the key file.
        for (seq, a) in b.iter() {
            assert!(a.name().is_some() || matches!(a, Action::SetMark(_) | Action::GoMark(_) | Action::NthCard(_)), "{}", display(seq));
        }
        // Ctrl+w alone waits, then closes.
        assert_eq!(feed("<C-w>"), Outcome::Wait);
        assert_eq!(p.expire(&b, t + TIMEOUT), Some(Outcome::Run { action: Action::Close, count: None }));
        // Keys that switch modes stay out of the table.
        for s in ["<Esc>", "<S-Esc>", "<Tab>", "<Enter>", "<Space>", "<C-f>", "?"] {
            assert_eq!(b.lookup(&parse(s).unwrap()), Lookup::None, "{s}");
        }
    }
}
