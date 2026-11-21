//! The `:` command line (M12 step 6): a line such as `open example.com` or
//! `space Work`, typed in the palette after a `:`. The command names and the
//! table are our own; a few short names (`q`, `w`, `e`) follow the common
//! vim-style habit.
//!
//! [`parse`] reads a line into a [`Parsed`] command: a name (or a unique
//! start of one, `tid` → `tidy`) and its argument. [`complete`] offers the
//! names, or an argument's values, that fit what is typed. [`Recent`] keeps
//! the lines run before, newest first.

use crate::palette::{self, Candidate};

/// What a command does; the app runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cmd {
    /// Open an address (or search for words) in this card, else a new one.
    Open,
    /// Open an address beside the current card.
    Beside,
    /// A new card: with an address, open it; without, ask for one.
    New,
    StartPage,
    Note,
    Sticky,
    Close,
    Exit,
    Save,
    Reload,
    Back,
    Forward,
    Find,
    Zoom,
    Copy,
    Tidy,
    Fit,
    Fold,
    Undo,
    Redo,
    Reopen,
    Solo,
    Sidebar,
    Card,
    CardNext,
    CardPrev,
    CardOnly,
    Clone,
    Saved,
    SavedBeside,
    SavePage,
    Mark,
    ToMark,
    JumpBack,
    JumpForward,
    Name,
    Colour,
    SelectAll,
    Space,
    SpaceNew,
    SpacePrivate,
    SpaceRename,
    SpaceDelete,
    Private,
    Tint,
    Photo,
    History,
    Downloads,
    Settings,
    Keys,
    Messages,
    Bind,
    Unbind,
    Alias,
    ReloadKeys,
    KeysFile,
    Run,
    Hint,
    HintMany,
    Welcome,
    Ads,
    AdsAllowed,
    ClearHistory,
    ClearCookies,
    Set,
    Reset,
    Toggle,
    Site,
    SiteSession,
    Up,
    Root,
    PageNext,
    PagePrev,
    Increment,
    Decrement,
    Stop,
    Home,
    ViewSource,
    SaveFile,
    Fullscreen,
    Later,
    Repeat,
}

/// What a command's argument is: it decides what [`complete`] offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An address or words to search for.
    Address,
    /// Free text (a name, words to find).
    Text,
    /// A space's name (the app gives the names).
    Space,
    /// A card's title (the app gives the titles).
    Card,
    /// A saved page's name (the app gives the names).
    Saved,
    /// A number of times.
    Count,
    /// One of these words (a unique start of one is enough).
    Choice(&'static [&'static str]),
    /// A setting's name, then values for it (`page-colours dark`); the
    /// app gives both (`values(Kind::Setting, "")` the names,
    /// `values(Kind::Setting, name)` its values).
    Setting,
    /// A setting's name only.
    SettingName,
    /// A site switch, then on / off (`javascript off`).
    Site,
}

/// Whether a command takes an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    None,
    Optional(Kind),
    Required(Kind),
}

impl Arg {
    pub fn kind(self) -> Option<Kind> {
        match self {
            Arg::None => None,
            Arg::Optional(k) | Arg::Required(k) => Some(k),
        }
    }
}

/// One command of the table.
#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub cmd: Cmd,
    pub name: &'static str,
    /// Short names that are exactly this command (`q` for `close`).
    pub aliases: &'static [&'static str],
    pub arg: Arg,
    /// How the argument is written in the list (`<address>`).
    pub arg_hint: &'static str,
    /// What it does, as the list says it.
    pub about: &'static str,
}

const ZOOM: &[&str] = &["in", "out", "reset"];
const COPY: &[&str] = &["address", "title", "markdown", "site", "clean"];
const MESSAGES: &[&str] = &["clear"];
const RELOAD: &[&str] = &["fresh"];

const fn s(cmd: Cmd, name: &'static str, aliases: &'static [&'static str], arg: Arg, arg_hint: &'static str, about: &'static str) -> Spec {
    Spec { cmd, name, aliases, arg, arg_hint, about }
}

use self::Arg::{None as No, Optional as Opt, Required as Req};

/// Every command, in the order the list shows them with nothing typed.
pub const COMMANDS: &[Spec] = &[
    s(Cmd::Open, "open", &["e"], Req(Kind::Address), "<address or words>", "Open in this card, else a new one"),
    s(Cmd::Beside, "beside", &[], Req(Kind::Address), "<address or words>", "Open beside the current card"),
    s(Cmd::New, "new", &[], Opt(Kind::Address), "[address]", "New card"),
    s(Cmd::StartPage, "start", &[], No, "", "New start page"),
    s(Cmd::Note, "note", &[], No, "", "New note"),
    s(Cmd::Sticky, "sticky", &[], No, "", "New sticky note"),
    s(Cmd::Card, "card", &[], Req(Kind::Card), "<title or number>", "Go to a card (a number counts in reading order)"),
    s(Cmd::CardNext, "card-next", &[], Opt(Kind::Count), "[times]", "Next card in reading order"),
    s(Cmd::CardPrev, "card-prev", &[], Opt(Kind::Count), "[times]", "Previous card in reading order"),
    s(Cmd::CardOnly, "card-only", &["only"], No, "", "Close the other cards in this space"),
    s(Cmd::Clone, "clone", &[], No, "", "Open the card's page again beside it"),
    s(Cmd::Saved, "saved", &[], Req(Kind::Saved), "<name>", "Open a saved page here (Shift+Delete forgets one)"),
    s(Cmd::SavedBeside, "saved-beside", &[], Req(Kind::Saved), "<name>", "Open a saved page beside the current card"),
    s(Cmd::SavePage, "save-page", &[], Opt(Kind::Text), "[name]", "Save this page under a name"),
    s(Cmd::Mark, "mark", &[], Req(Kind::Text), "<letter>", "Mark this spot (a–z: the page's scroll, A–Z: the board)"),
    s(Cmd::ToMark, "to-mark", &[], Req(Kind::Text), "<letter>", "Go to a mark (' : where the last jump left)"),
    s(Cmd::JumpBack, "jump-back", &[], Opt(Kind::Count), "[times]", "Back through the cards visited"),
    s(Cmd::JumpForward, "jump-forward", &[], Opt(Kind::Count), "[times]", "Forward through the cards visited"),
    s(Cmd::Space, "space", &[], Req(Kind::Space), "<name>", "Go to a space"),
    s(Cmd::Close, "close", &["q"], No, "", "Close the card (or the selection)"),
    s(Cmd::Reopen, "reopen", &[], No, "", "Reopen the last closed card"),
    s(Cmd::Back, "back", &[], Opt(Kind::Count), "[times]", "Go back in the card's page"),
    s(Cmd::Forward, "forward", &[], Opt(Kind::Count), "[times]", "Go forward in the card's page"),
    s(Cmd::Reload, "reload", &[], Opt(Kind::Choice(RELOAD)), "[fresh]", "Reload the page (fresh: skip the cache)"),
    s(Cmd::Find, "find", &[], Opt(Kind::Text), "[words]", "Find in the page"),
    s(Cmd::Copy, "copy", &[], Opt(Kind::Choice(COPY)), "[address|title|markdown|site|clean]", "Copy the page's address, title, a Markdown link, its site, or the address without tracking"),
    s(Cmd::PageNext, "page-next", &[], No, "", "The page's next page (its next link)"),
    s(Cmd::PagePrev, "page-prev", &[], No, "", "The page's previous page"),
    s(Cmd::Up, "up", &[], Opt(Kind::Count), "[times]", "Up the address's path"),
    s(Cmd::Root, "root", &[], No, "", "To the site's root"),
    s(Cmd::Increment, "increment", &[], Opt(Kind::Count), "[by]", "The address's last number up"),
    s(Cmd::Decrement, "decrement", &[], Opt(Kind::Count), "[by]", "The address's last number down"),
    s(Cmd::Stop, "stop", &[], No, "", "Stop loading the page"),
    s(Cmd::Home, "home", &[], No, "", "A start page beside the card"),
    s(Cmd::ViewSource, "view-source", &[], No, "", "The page's source in a card beside"),
    s(Cmd::SaveFile, "save-file", &[], No, "", "Download the page as a file"),
    s(Cmd::Fullscreen, "fullscreen", &[], No, "", "Full screen on or off"),
    s(Cmd::Zoom, "zoom", &[], Req(Kind::Choice(ZOOM)), "<in|out|reset>", "Zoom the page"),
    s(Cmd::Save, "save", &["w"], No, "", "Save as Markdown"),
    s(Cmd::Solo, "solo", &[], No, "", "Solo on or off"),
    s(Cmd::Tidy, "tidy", &[], No, "", "Tidy trails"),
    s(Cmd::Fit, "fit", &[], No, "", "Fit everything in view"),
    s(Cmd::Fold, "fold", &[], No, "", "Fold or unfold the trail"),
    s(Cmd::Name, "name", &[], Opt(Kind::Text), "[name]", "Name the trail"),
    s(Cmd::Colour, "colour", &["color"], No, "", "Next trail colour"),
    s(Cmd::SelectAll, "select-all", &[], No, "", "Select every card"),
    s(Cmd::Undo, "undo", &["u"], No, "", "Undo"),
    s(Cmd::Redo, "redo", &[], No, "", "Redo"),
    s(Cmd::Sidebar, "sidebar", &[], No, "", "Trails sidebar"),
    s(Cmd::SpaceNew, "space-new", &[], Opt(Kind::Text), "[name]", "New space"),
    s(Cmd::SpacePrivate, "space-private", &[], Opt(Kind::Text), "[name]", "New private space"),
    s(Cmd::SpaceRename, "space-rename", &[], Req(Kind::Text), "<name>", "Rename this space"),
    s(Cmd::SpaceDelete, "space-delete", &[], No, "", "Delete this space"),
    s(Cmd::Private, "private", &[], No, "", "This space private or recorded"),
    s(Cmd::Tint, "tint", &[], No, "", "Next space tint"),
    s(Cmd::Photo, "photo", &[], No, "", "Background photo"),
    s(Cmd::History, "history", &[], Opt(Kind::Text), "[words]", "History"),
    s(Cmd::Downloads, "downloads", &[], No, "", "Downloads"),
    s(Cmd::Settings, "settings", &[], No, "", "Settings"),
    s(Cmd::Set, "set", &[], Req(Kind::Setting), "<setting> [value]", "Change a setting (no value: show it)"),
    s(Cmd::Reset, "reset", &[], Req(Kind::SettingName), "<setting>", "A setting back to its default"),
    s(Cmd::Toggle, "toggle", &[], Req(Kind::Setting), "<setting> [values…]", "Flip a setting, or go round the values given"),
    s(Cmd::Site, "site", &[], Req(Kind::Site), "<javascript|images|cookies|ads|passthrough> [on|off]", "Switch something for this site (saved)"),
    s(Cmd::SiteSession, "site-session", &[], Req(Kind::Site), "<javascript|images|cookies|ads|passthrough> [on|off]", "Switch something for this site until you quit"),
    s(Cmd::Keys, "keys", &["help"], No, "", "Keyboard shortcuts"),
    s(Cmd::Messages, "messages", &[], Opt(Kind::Choice(MESSAGES)), "[clear]", "Messages shown lately, and lines waiting to run (clear: forget the messages)"),
    s(Cmd::Bind, "bind", &[], Req(Kind::Text), "<modes> <keys> <command>", "Bind keys until restart (modes: board,page,insert,hints,command)"),
    s(Cmd::Unbind, "unbind", &[], Req(Kind::Text), "<modes> <keys>", "Unbind keys until restart"),
    s(Cmd::Alias, "alias", &[], Req(Kind::Text), "<name> <command>", "A short name for a command, until restart"),
    s(Cmd::ReloadKeys, "reload-keys", &[], No, "", "Read the keys file again"),
    s(Cmd::KeysFile, "keys-file", &[], No, "", "Open the keys file in a card"),
    s(Cmd::Run, "run", &[], Req(Kind::Text), "<program> [words]", "Start a program ({url}, {title}, {hint-url} filled in)"),
    s(Cmd::Hint, "hint", &[], Req(Kind::Text), "<command>", "Label links; the one chosen runs the command ({hint-url})"),
    s(Cmd::HintMany, "hint-many", &[], Req(Kind::Text), "<command>", "Like hint, for many links until Esc"),
    s(Cmd::Welcome, "welcome", &[], No, "", "Welcome page"),
    s(Cmd::Ads, "ads", &[], No, "", "Allow or block ads on this site"),
    s(Cmd::AdsAllowed, "ads-allowed", &[], No, "", "Sites with ads allowed"),
    s(Cmd::ClearHistory, "clear-history", &[], No, "", "Clear history"),
    s(Cmd::ClearCookies, "clear-cookies", &[], No, "", "Clear cookies"),
    s(Cmd::Later, "later", &[], Req(Kind::Text), "<ms> <command>", "Run a command after a wait, in milliseconds"),
    s(Cmd::Repeat, "repeat", &[], Req(Kind::Text), "<times> <command>", "Run a command several times"),
    s(Cmd::Exit, "exit", &["qa"], No, "", "Quit Trailboard"),
];

/// A line read as a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub spec: Spec,
    /// The argument, trimmed; a choice is its full word (`o` → `out`).
    pub arg: Option<String>,
}

impl PartialEq for Spec {
    fn eq(&self, other: &Self) -> bool {
        self.cmd == other.cmd
    }
}
impl Eq for Spec {}

impl Parsed {
    /// A count argument, if there is one.
    pub fn count(&self) -> Option<u32> {
        self.arg.as_deref().and_then(|a| a.parse().ok())
    }

    /// The line written out in full (`zoom out`), for history.
    pub fn line(&self) -> String {
        match &self.arg {
            Some(a) => format!("{} {a}", self.spec.name),
            None => self.spec.name.to_string(),
        }
    }
}

/// Why a line isn't a command, as the list says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Empty,
    Unknown(String),
    /// Starts more than one name: these.
    Ambiguous(Vec<&'static str>),
    /// The command needs an argument: what it is.
    Missing(&'static str, &'static str),
    /// The command takes none.
    NoArgument(&'static str),
    /// Not one of the choices, or not a number.
    BadArgument(&'static str, String, &'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Empty => write!(f, "Type a command"),
            Error::Unknown(n) => write!(f, "No command “{n}”"),
            Error::Ambiguous(names) => write!(f, "Could be {}", names.join(", ")),
            Error::Missing(n, hint) => write!(f, "{n} needs {hint}"),
            Error::NoArgument(n) => write!(f, "{n} takes nothing after it"),
            Error::BadArgument(n, a, hint) => write!(f, "{n}: “{a}” isn't {hint}"),
        }
    }
}

/// The line without its `:` and outer spaces, split into the name and the rest.
fn split(line: &str) -> (&str, &str) {
    let line = line.trim_start().strip_prefix(':').unwrap_or(line).trim();
    match line.split_once(char::is_whitespace) {
        Some((name, rest)) => (name, rest.trim()),
        None => (line, ""),
    }
}

/// The command `name` stands for: a name or alias exactly (case ignored),
/// else the one name it starts.
pub fn lookup(name: &str) -> Result<Spec, Error> {
    if name.is_empty() {
        return Err(Error::Empty);
    }
    let lower = name.to_lowercase();
    if let Some(s) = COMMANDS.iter().find(|s| s.name == lower || s.aliases.contains(&lower.as_str())) {
        return Ok(*s);
    }
    let starts: Vec<&Spec> = COMMANDS.iter().filter(|s| s.name.starts_with(&lower)).collect();
    match starts.as_slice() {
        [one] => Ok(**one),
        [] => Err(Error::Unknown(name.to_string())),
        many => Err(Error::Ambiguous(many.iter().map(|s| s.name).collect())),
    }
}

/// The choice `arg` stands for: a word exactly, else the one it starts.
fn choose(choices: &'static [&'static str], arg: &str) -> Option<&'static str> {
    let lower = arg.to_lowercase();
    if let Some(c) = choices.iter().find(|c| **c == lower) {
        return Some(c);
    }
    let mut starts = choices.iter().filter(|c| c.starts_with(&lower));
    match (starts.next(), starts.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// Reads a line (with or without its leading `:`).
pub fn parse(line: &str) -> Result<Parsed, Error> {
    let (name, rest) = split(line);
    let spec = lookup(name)?;
    let arg = match (spec.arg, rest.is_empty()) {
        (Arg::None, true) | (Arg::Optional(_), true) => None,
        (Arg::None, false) => return Err(Error::NoArgument(spec.name)),
        (Arg::Required(_), true) => return Err(Error::Missing(spec.name, spec.arg_hint)),
        (Arg::Optional(k) | Arg::Required(k), false) => Some(match k {
            Kind::Choice(choices) => choose(choices, rest)
                .ok_or_else(|| Error::BadArgument(spec.name, rest.to_string(), spec.arg_hint))?
                .to_string(),
            Kind::Count => match rest.parse::<u32>() {
                Ok(n) if n > 0 => n.to_string(),
                _ => return Err(Error::BadArgument(spec.name, rest.to_string(), "a number")),
            },
            _ => rest.to_string(),
        }),
    };
    Ok(Parsed { spec, arg })
}

/// The most a count, or `:repeat`, runs a command.
pub const MAX_TIMES: u32 = 100;

/// The longest `:later` waits: a day, in milliseconds.
pub const MAX_LATER_MS: u64 = 24 * 60 * 60 * 1000;

/// A count before a line (`3 back`, `3back`, `:3 back`): the count and
/// the line after it. A number and a space is a count with the command
/// still to come (`3:` opens the line so); a number alone keeps its digits.
pub fn split_count(line: &str) -> (Option<u32>, &str) {
    let bare = line.trim_start().strip_prefix(':').unwrap_or(line.trim_start()).trim_start();
    let digits = bare.len() - bare.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let rest = bare[digits..].trim_start();
    if digits == 0 || digits == bare.len() {
        return (None, bare);
    }
    match bare[..digits].parse::<u32>() {
        Ok(n) if n > 0 => (Some(n), rest),
        _ => (None, bare),
    }
}

/// `:later` and `:repeat`'s argument: a number, then the command to run.
/// `what` names the number in the message (`milliseconds`, `times`).
pub fn split_times(arg: &str, what: &str) -> Result<(u64, String), String> {
    let (n, line) = arg.trim().split_once(char::is_whitespace).map_or((arg.trim(), ""), |(n, l)| (n, l.trim()));
    let n: u64 = n.parse().map_err(|_| format!("“{n}” isn't a number of {what}"))?;
    if line.is_empty() {
        return Err(format!("a command goes after the {what}"));
    }
    Ok((n, line.to_string()))
}

/// Something the list offers for what is typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// The line it makes (no `:`); Tab puts it in the field.
    pub line: String,
    /// What the row shows: the name and how its argument is written, or a value.
    pub label: String,
    /// Shown dim on the right.
    pub detail: String,
    /// Matched letters of `label` (char indices).
    pub positions: Vec<usize>,
}

impl Completion {
    /// It is a whole command: Enter on it runs it (else Enter fills it in).
    pub fn runs(&self) -> bool {
        parse(&self.line).is_ok()
    }
}

/// What gives an argument's values: for a kind (and, for a setting's
/// values, the setting's name), (value, what the row says on its right).
pub type Values<'a> = dyn Fn(Kind, &str) -> Vec<(String, String)> + 'a;

/// Values the list offers for a command's argument, at most.
pub const MAX_VALUES: usize = 30;

/// What fits `line`: while the name is typed, the commands it finds (names
/// it starts first, then fuzzy matches, then those found by what they do);
/// after the name and a space, the values of its argument that fit
/// (`values` gives a space's or card's names, or saved pages', each with
/// what the row says on its right, empty for what the command does;
/// choices come from the table).
pub fn complete(line: &str, values: &Values) -> Vec<Completion> {
    let bare = line.trim_start().strip_prefix(':').unwrap_or(line).trim_start();
    let typing_name = !bare.contains(char::is_whitespace);
    if typing_name {
        return complete_name(bare);
    }
    let (name, rest) = split(bare);
    let Ok(spec) = lookup(name) else { return Vec::new() };
    let Some(kind) = spec.arg.kind() else { return Vec::new() };
    // Two-word arguments: the words before the last one are fixed, the
    // last is completed.
    let (fixed, rest) = match kind {
        Kind::Setting | Kind::SettingName | Kind::Site if line.ends_with(char::is_whitespace) && !rest.is_empty() => (rest, ""),
        Kind::Setting | Kind::SettingName | Kind::Site => match rest.rsplit_once(char::is_whitespace) {
            Some((before, last)) => (before.trim(), last),
            None => ("", rest),
        },
        _ => ("", rest),
    };
    let words = |w: &[&str]| w.iter().map(|s| (s.to_string(), String::new())).collect::<Vec<_>>();
    let offered: Vec<(String, String)> = match kind {
        Kind::Choice(c) => words(c),
        Kind::Space | Kind::Card | Kind::Saved => values(kind, ""),
        Kind::SettingName if fixed.is_empty() => values(Kind::Setting, ""),
        Kind::Setting => values(Kind::Setting, fixed.split_whitespace().next().unwrap_or("")),
        Kind::Site if fixed.is_empty() => words(&crate::setcmd::SITE_WORDS),
        Kind::Site if !fixed.contains(char::is_whitespace) => words(&["on", "off"]),
        _ => return Vec::new(),
    };
    let lead = if fixed.is_empty() { spec.name.to_string() } else { format!("{} {fixed}", spec.name) };
    let cands: Vec<Candidate> = offered.iter().map(|(v, _)| Candidate { text: v, alt: "" }).collect();
    palette::rank(rest, &cands)
        .into_iter()
        .take(MAX_VALUES)
        .map(|r| {
            let (v, detail) = &offered[r.index];
            let detail = if detail.is_empty() { spec.about.to_string() } else { detail.clone() };
            Completion { line: format!("{lead} {v}"), label: v.clone(), detail, positions: r.positions }
        })
        .collect()
}

fn complete_name(typed: &str) -> Vec<Completion> {
    let typed = typed.to_lowercase();
    let row = |s: &Spec, positions: Vec<usize>| {
        let label = if s.arg_hint.is_empty() { s.name.to_string() } else { format!("{} {}", s.name, s.arg_hint) };
        let also = if s.aliases.is_empty() { String::new() } else { format!(" · also {}", s.aliases.join(", ")) };
        let line = if s.arg == Arg::None { s.name.to_string() } else { format!("{} ", s.name) };
        Completion { line, label, detail: format!("{}{also}", s.about), positions }
    };
    if typed.is_empty() {
        return COMMANDS.iter().map(|s| row(s, Vec::new())).collect();
    }
    let mut out: Vec<Completion> = Vec::new();
    let mut taken = vec![false; COMMANDS.len()];
    // An alias typed exactly, then names it starts, in table order.
    for (i, s) in COMMANDS.iter().enumerate() {
        if s.aliases.contains(&typed.as_str()) {
            out.push(row(s, Vec::new()));
            taken[i] = true;
        }
    }
    for (i, s) in COMMANDS.iter().enumerate() {
        if !taken[i] && s.name.starts_with(&typed) {
            out.push(row(s, (0..typed.chars().count()).collect()));
            taken[i] = true;
        }
    }
    // Then the rest by fuzzy match on the name, or on what it does.
    let cands: Vec<Candidate> = COMMANDS.iter().map(|s| Candidate { text: s.name, alt: s.about }).collect();
    for r in palette::rank(&typed, &cands) {
        if !taken[r.index] {
            out.push(row(&COMMANDS[r.index], r.positions));
            taken[r.index] = true;
        }
    }
    out
}

/// Lines kept in [`Recent`].
pub const MAX_RECENT: usize = 100;

/// Lines run before, newest first, each once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recent {
    lines: Vec<String>,
}

impl Recent {
    pub fn from_lines(lines: Vec<String>) -> Recent {
        let mut r = Recent::default();
        for l in lines.into_iter().rev() {
            r.push(&l);
        }
        r
    }

    /// Newest first.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Notes that `line` ran: it moves to the front.
    pub fn push(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        self.lines.retain(|l| l != line);
        self.lines.insert(0, line.to_string());
        self.lines.truncate(MAX_RECENT);
    }

    /// Drops `line`; true if it was there.
    pub fn forget(&mut self, line: &str) -> bool {
        let before = self.lines.len();
        self.lines.retain(|l| l != line);
        self.lines.len() != before
    }

    /// Lines that start with what is typed (no `:`, case ignored), newest first.
    pub fn matching<'a>(&'a self, typed: &str) -> impl Iterator<Item = &'a String> {
        let typed = typed.trim_start().strip_prefix(':').unwrap_or(typed).trim_start().to_lowercase();
        self.lines.iter().filter(move |l| l.to_lowercase().starts_with(&typed))
    }
}

/// Up / Down in the command line: a walk through the lines run before
/// that start with what was typed when it began, newest first. Step 0 is
/// the typed text itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Walk {
    typed: String,
    at: usize,
}

impl Walk {
    /// A walk from `typed` (with or without its `:`).
    pub fn new(typed: &str) -> Walk {
        Walk { typed: typed.trim_start().strip_prefix(':').unwrap_or(typed).to_string(), at: 0 }
    }

    /// One step older (`older`) or newer: the line to show (no `:`), or
    /// `None` at the oldest match, where the field stays as it is.
    pub fn step(&mut self, recent: &Recent, older: bool) -> Option<String> {
        let typed = self.typed.trim();
        let lines: Vec<&String> = recent.matching(&self.typed).filter(|l| !l.eq_ignore_ascii_case(typed)).collect();
        let to = if older { self.at + 1 } else { self.at.checked_sub(1)? };
        if to > lines.len() {
            return None;
        }
        self.at = to;
        Some(if to == 0 { self.typed.clone() } else { lines[to - 1].clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: Kind, _: &str) -> Vec<(String, String)> {
        Vec::new()
    }

    #[test]
    fn table_is_sound() {
        let mut seen = std::collections::HashSet::new();
        for s in COMMANDS {
            assert!(seen.insert(s.name), "name {} twice", s.name);
            for a in s.aliases {
                assert!(seen.insert(a), "alias {a} twice");
            }
            assert!(!s.name.contains(' '), "{}", s.name);
            assert_eq!(s.arg == Arg::None, s.arg_hint.is_empty(), "{}", s.name);
        }
    }

    #[test]
    fn names_aliases_and_starts() {
        assert_eq!(parse(":tidy").unwrap().spec.cmd, Cmd::Tidy);
        assert_eq!(parse("  :TIDY  ").unwrap().spec.cmd, Cmd::Tidy);
        assert_eq!(parse("tid").unwrap().spec.cmd, Cmd::Tidy);
        assert_eq!(parse("q").unwrap().spec.cmd, Cmd::Close);
        assert_eq!(parse("qa").unwrap().spec.cmd, Cmd::Exit);
        assert_eq!(parse("w").unwrap().spec.cmd, Cmd::Save);
        assert_eq!(parse("e example.com").unwrap().spec.cmd, Cmd::Open);
        // An exact name wins over longer names it starts.
        assert_eq!(parse("space Work").unwrap().spec.cmd, Cmd::Space);
        assert_eq!(parse("ads").unwrap().spec.cmd, Cmd::Ads);
        assert_eq!(parse("t"), Err(Error::Ambiguous(vec!["to-mark", "tidy", "tint", "toggle"])));
        // Step 13: later and repeat; `rep` is enough, `re` isn't.
        assert_eq!(parse("rep 3 back").unwrap().spec.cmd, Cmd::Repeat);
        assert_eq!(parse("later 500 reload").unwrap().arg.as_deref(), Some("500 reload"));
        assert!(matches!(parse("re"), Err(Error::Ambiguous(_))));
        // Settings (step 11): `set` is exact, `sett` the settings page.
        assert_eq!(parse("set page-colours dark").unwrap().arg.as_deref(), Some("page-colours dark"));
        assert_eq!(parse("sett").unwrap().spec.cmd, Cmd::Settings);
        assert_eq!(parse("site js off").unwrap().spec.cmd, Cmd::Site);
        assert_eq!(parse("site-s cookies").unwrap().spec.cmd, Cmd::SiteSession);
        assert!(matches!(parse("reset"), Err(Error::Missing(..))));
        // Saved pages, marks and jumps (step 9); `save` stays Save as Markdown.
        assert_eq!(parse("save").unwrap().spec.cmd, Cmd::Save);
        assert!(matches!(parse("saved"), Err(Error::Missing(..))), "Enter on it lists the names");
        assert_eq!(parse("saved Rust docs").unwrap().arg.as_deref(), Some("Rust docs"));
        assert_eq!(parse("saved-b x").unwrap().spec.cmd, Cmd::SavedBeside);
        assert!(matches!(parse("saved-beside"), Err(Error::Missing(..))));
        assert_eq!(parse("save-page").unwrap().spec.cmd, Cmd::SavePage);
        assert_eq!(parse("sav"), Err(Error::Ambiguous(vec!["saved", "saved-beside", "save-page", "save-file", "save"])));
        // Navigation (step 12): names that keep the old short starts working.
        assert_eq!(parse("so").unwrap().spec.cmd, Cmd::Solo);
        assert_eq!(parse("ne").unwrap().spec.cmd, Cmd::New);
        assert_eq!(parse("up 2").unwrap().count(), Some(2));
        assert_eq!(parse("pa").map_err(|e| e.to_string()), Err("Could be page-next, page-prev".into()));
        assert_eq!(parse("v").unwrap().spec.cmd, Cmd::ViewSource);
        assert_eq!(parse("copy cl").unwrap().arg.as_deref(), Some("clean"));
        assert_eq!(parse("copy si").unwrap().arg.as_deref(), Some("site"));
        assert_eq!(parse("mark A").unwrap().arg.as_deref(), Some("A"));
        assert_eq!(parse("jump-b 3").unwrap().count(), Some(3));
        // `card` is exact; its longer names need more.
        assert_eq!(parse("card 3").unwrap().spec.cmd, Cmd::Card);
        assert_eq!(parse("card-n").unwrap().spec.cmd, Cmd::CardNext);
        assert_eq!(parse("card-next 2").unwrap().count(), Some(2));
        assert_eq!(parse("only").unwrap().spec.cmd, Cmd::CardOnly);
        assert_eq!(parse("clo"), Err(Error::Ambiguous(vec!["clone", "close"])));
        assert_eq!(parse("nope"), Err(Error::Unknown("nope".into())));
        assert_eq!(parse(":"), Err(Error::Empty));
        assert_eq!(parse(""), Err(Error::Empty));
    }

    #[test]
    fn arguments() {
        let p = parse("open  rust lang  ").unwrap();
        assert_eq!(p.arg.as_deref(), Some("rust lang"));
        assert_eq!(p.line(), "open rust lang");
        assert_eq!(parse("open"), Err(Error::Missing("open", "<address or words>")));
        assert_eq!(parse("tidy now"), Err(Error::NoArgument("tidy")));
        assert_eq!(parse("new").unwrap().arg, None);
        assert_eq!(parse("zoom o").unwrap().arg.as_deref(), Some("out"));
        assert_eq!(parse("zoom O").unwrap().line(), "zoom out");
        assert!(matches!(parse("zoom sideways"), Err(Error::BadArgument("zoom", _, _))));
        assert_eq!(parse("back 3").unwrap().count(), Some(3));
        assert_eq!(parse("back").unwrap().count(), None);
        assert!(parse("back 0").is_err());
        assert!(parse("back x").is_err());
        assert_eq!(parse("reload f").unwrap().arg.as_deref(), Some("fresh"));
        assert_eq!(parse("copy t").unwrap().arg.as_deref(), Some("title"));
        assert_eq!(Error::Missing("open", "<address or words>").to_string(), "open needs <address or words>");
    }

    #[test]
    fn completes_names() {
        let all = complete(":", &none);
        assert_eq!(all.len(), COMMANDS.len());
        assert_eq!(all[0].line, "open ");
        assert!(!all[0].runs(), "open needs an address");
        let c = complete(":sp", &none);
        let names: Vec<&str> = c.iter().map(|c| c.line.trim()).collect();
        assert_eq!(&names[..5], &["space", "space-new", "space-private", "space-rename", "space-delete"]);
        assert_eq!(c[0].label, "space <name>");
        assert_eq!(c[0].positions, vec![0, 1]);
        // An alias first, then the names it starts.
        let c = complete("q", &none);
        assert_eq!(c[0].line, "close");
        assert!(c[0].detail.contains("also q"));
        assert!(c[0].runs());
        // Found by what it does, too.
        assert!(complete("quit", &none).iter().any(|c| c.line == "exit"));
    }

    #[test]
    fn completes_values() {
        let values = |k: Kind, _: &str| match k {
            Kind::Space => ["Home", "Work", "Reading list"].iter().map(|s| (s.to_string(), String::new())).collect(),
            Kind::Saved => vec![("Rust docs".to_string(), "doc.rust-lang.org".to_string())],
            _ => Vec::new(),
        };
        let c = complete("saved ", &values);
        assert_eq!((c[0].line.as_str(), c[0].detail.as_str()), ("saved Rust docs", "doc.rust-lang.org"));
        assert_eq!(complete("saved-beside ru", &values)[0].line, "saved-beside Rust docs");
        let c = complete(":space ", &values);
        assert_eq!(c.iter().map(|c| c.line.as_str()).collect::<Vec<_>>(), vec!["space Home", "space Work", "space Reading list"]);
        let c = complete(":space rl", &values);
        assert_eq!(c[0].line, "space Reading list");
        assert!(c[0].runs());
        let c = complete("zoom ", &values);
        assert_eq!(c.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), vec!["in", "out", "reset"]);
        // An address or free text has nothing to offer; neither has a bad name.
        assert!(complete("open ex", &values).is_empty());
        assert!(complete("nope x", &values).is_empty());
        assert!(complete("tidy ", &values).is_empty());
    }

    #[test]
    fn completes_settings_and_sites() {
        let values = |k: Kind, name: &str| match (k, name) {
            (Kind::Setting, "") => vec![("page-colours".to_string(), "Page colours: Dark".to_string()), ("history".to_string(), String::new())],
            (Kind::Setting, "page-colours") => vec![("theme".to_string(), String::new()), ("dark".to_string(), "Dark · now".to_string())],
            _ => Vec::new(),
        };
        let c = complete("set pa", &values);
        assert_eq!((c[0].line.as_str(), c[0].detail.as_str()), ("set page-colours", "Page colours: Dark"));
        let c = complete("set page-colours ", &values);
        assert_eq!(c.iter().map(|c| c.line.as_str()).collect::<Vec<_>>(), vec!["set page-colours theme", "set page-colours dark"]);
        assert_eq!(complete("toggle page-colours dark t", &values)[0].line, "toggle page-colours dark theme");
        assert_eq!(complete("reset hi", &values)[0].line, "reset history");
        assert!(complete("reset history ", &values).is_empty(), "reset takes the name only");
        let c = complete("site ", &values);
        assert_eq!(c.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), vec!["javascript", "images", "cookies", "ads", "passthrough"]);
        assert_eq!(complete("site-session co", &values)[0].line, "site-session cookies");
        let c = complete("site cookies ", &values);
        assert_eq!(c.iter().map(|c| c.line.as_str()).collect::<Vec<_>>(), vec!["site cookies on", "site cookies off"]);
        assert!(complete("site cookies off ", &values).is_empty());
    }

    #[test]
    fn recent_lines() {
        let mut r = Recent::default();
        r.push("tidy");
        r.push("open a.com");
        r.push("  tidy ");
        r.push("");
        assert_eq!(r.lines(), &["tidy".to_string(), "open a.com".to_string()]);
        assert_eq!(r.matching(":op").collect::<Vec<_>>(), vec!["open a.com"]);
        assert_eq!(r.matching("").count(), 2);
        for i in 0..150 {
            r.push(&format!("find {i}"));
        }
        assert_eq!(r.lines().len(), MAX_RECENT);
        assert_eq!(r.lines()[0], "find 149");
        assert!(r.forget("find 149"));
        assert!(!r.forget("find 149"));
        assert_eq!(r.lines()[0], "find 148");
        let back = Recent::from_lines(r.lines().to_vec());
        assert_eq!(back, r);
    }

    #[test]
    fn counts_before_a_line() {
        assert_eq!(split_count("3 back"), (Some(3), "back"));
        assert_eq!(split_count(":3back"), (Some(3), "back"));
        assert_eq!(split_count("  :12  card-next 2"), (Some(12), "card-next 2"));
        assert_eq!(split_count(":back 3"), (None, "back 3"));
        assert_eq!(split_count(":3"), (None, "3"), "a number alone isn't a count");
        assert_eq!(split_count(":3 "), (Some(3), ""), "the command is still to come");
        assert_eq!(split_count("0 back"), (None, "0 back"));
        assert_eq!(split_count("99999999999 back"), (None, "99999999999 back"));
        assert_eq!(split_count(""), (None, ""));
    }

    #[test]
    fn times_then_a_command() {
        assert_eq!(split_times("500 reload", "milliseconds"), Ok((500, "reload".to_string())));
        assert_eq!(split_times(" 3  back | tidy ", "times"), Ok((3, "back | tidy".to_string())));
        assert_eq!(split_times("soon reload", "milliseconds"), Err("“soon” isn't a number of milliseconds".into()));
        assert_eq!(split_times("3", "times"), Err("a command goes after the times".into()));
    }

    #[test]
    fn walking_earlier_lines() {
        let r = Recent::from_lines(vec!["space Work".into(), "tidy".into(), "space Home".into(), "sp".into()]);
        let mut w = Walk::new(":sp");
        assert_eq!(w.step(&r, false), None, "already at the typed text");
        assert_eq!(w.step(&r, true).as_deref(), Some("space Work"));
        assert_eq!(w.step(&r, true).as_deref(), Some("space Home"), "tidy doesn't start with sp; sp itself is skipped");
        assert_eq!(w.step(&r, true), None, "the oldest stays");
        assert_eq!(w.step(&r, false).as_deref(), Some("space Work"));
        assert_eq!(w.step(&r, false).as_deref(), Some("sp"), "back to what was typed");
        let mut all = Walk::new(":");
        assert_eq!(all.step(&r, true).as_deref(), Some("space Work"));
        assert_eq!(all.step(&r, true).as_deref(), Some("tidy"));
        assert_eq!(Walk::new("x").step(&r, true), None);
    }
}
