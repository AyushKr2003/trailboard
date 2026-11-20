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
    Welcome,
    Ads,
    AdsAllowed,
    ClearHistory,
    ClearCookies,
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
    /// A number of times.
    Count,
    /// One of these words (a unique start of one is enough).
    Choice(&'static [&'static str]),
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
const COPY: &[&str] = &["address", "title", "markdown"];
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
    s(Cmd::Card, "card", &[], Req(Kind::Card), "<title>", "Go to a card"),
    s(Cmd::Space, "space", &[], Req(Kind::Space), "<name>", "Go to a space"),
    s(Cmd::Close, "close", &["q"], No, "", "Close the card (or the selection)"),
    s(Cmd::Reopen, "reopen", &[], No, "", "Reopen the last closed card"),
    s(Cmd::Back, "back", &[], Opt(Kind::Count), "[times]", "Go back in the card's page"),
    s(Cmd::Forward, "forward", &[], Opt(Kind::Count), "[times]", "Go forward in the card's page"),
    s(Cmd::Reload, "reload", &[], Opt(Kind::Choice(RELOAD)), "[fresh]", "Reload the page (fresh: skip the cache)"),
    s(Cmd::Find, "find", &[], Opt(Kind::Text), "[words]", "Find in the page"),
    s(Cmd::Copy, "copy", &[], Opt(Kind::Choice(COPY)), "[address|title|markdown]", "Copy the page's address, title or a Markdown link"),
    s(Cmd::Zoom, "zoom", &[], Req(Kind::Choice(ZOOM)), "<in|out|reset>", "Zoom the page"),
    s(Cmd::Save, "save", &["w"], No, "", "Save as Markdown"),
    s(Cmd::Solo, "solo", &[], No, "", "Solo on or off"),
    s(Cmd::Tidy, "tidy", &[], No, "", "Tidy trails"),
    s(Cmd::Fit, "fit", &[], No, "", "Fit everything in view"),
    s(Cmd::Fold, "fold", &[], No, "", "Fold or unfold the trail"),
    s(Cmd::Name, "name", &[], Opt(Kind::Text), "[name]", "Name the trail"),
    s(Cmd::Colour, "colour", &["color"], No, "", "Next trail colour"),
    s(Cmd::SelectAll, "select-all", &[], No, "", "Select every card"),
    s(Cmd::Undo, "undo", &[], No, "", "Undo"),
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
    s(Cmd::Keys, "keys", &["help"], No, "", "Keyboard shortcuts"),
    s(Cmd::Welcome, "welcome", &[], No, "", "Welcome page"),
    s(Cmd::Ads, "ads", &[], No, "", "Allow or block ads on this site"),
    s(Cmd::AdsAllowed, "ads-allowed", &[], No, "", "Sites with ads allowed"),
    s(Cmd::ClearHistory, "clear-history", &[], No, "", "Clear history"),
    s(Cmd::ClearCookies, "clear-cookies", &[], No, "", "Clear cookies"),
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

/// Values the list offers for a command's argument, at most.
pub const MAX_VALUES: usize = 30;

/// What fits `line`: while the name is typed, the commands it finds (names
/// it starts first, then fuzzy matches, then those found by what they do);
/// after the name and a space, the values of its argument that fit
/// (`values` gives a space's or card's names; choices come from the table).
pub fn complete(line: &str, values: &dyn Fn(Kind) -> Vec<String>) -> Vec<Completion> {
    let bare = line.trim_start().strip_prefix(':').unwrap_or(line).trim_start();
    let typing_name = !bare.contains(char::is_whitespace);
    if typing_name {
        return complete_name(bare);
    }
    let (name, rest) = split(bare);
    let Ok(spec) = lookup(name) else { return Vec::new() };
    let Some(kind) = spec.arg.kind() else { return Vec::new() };
    let offered: Vec<String> = match kind {
        Kind::Choice(c) => c.iter().map(|s| s.to_string()).collect(),
        Kind::Space | Kind::Card => values(kind),
        _ => return Vec::new(),
    };
    let cands: Vec<Candidate> = offered.iter().map(|v| Candidate { text: v, alt: "" }).collect();
    palette::rank(rest, &cands)
        .into_iter()
        .take(MAX_VALUES)
        .map(|r| {
            let v = &offered[r.index];
            Completion { line: format!("{} {v}", spec.name), label: v.clone(), detail: spec.about.to_string(), positions: r.positions }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: Kind) -> Vec<String> {
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
        assert_eq!(parse("t"), Err(Error::Ambiguous(vec!["tidy", "tint"])));
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
        let values = |k: Kind| match k {
            Kind::Space => vec!["Home".to_string(), "Work".to_string(), "Reading list".to_string()],
            _ => Vec::new(),
        };
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
}
