//! The user's own keys (M12 step 10): bindings from the key file and
//! `:bind`, aliases, chains of commands, the words `:run` starts a program
//! with, and the rows the `?` sheet and the prefix panel show. Our own
//! design; `storage::keyfile` reads the file into [`Entry`]s.
//!
//! A binding's target is a *chain*: commands separated by `|` (`\|` is a
//! bar itself). Each is an action's name ([`keys::NAMED`], `scroll-down`),
//! an alias, or a command line (`open {url}`).

use crate::command;
use crate::keys::{self, Action, Bindings, Chord, Key};

/// Which keys a binding is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// No page has the keys.
    Board,
    /// A card entered: our page keys first.
    Page,
    /// Typing in a page's field (or `i`).
    Insert,
    /// While link hints show.
    Hints,
    /// In the `:` command line.
    Command,
    /// Caret and Visual mode (`v`, `V` in a page).
    Visual,
}

impl Mode {
    pub const ALL: [Mode; 6] = [Mode::Board, Mode::Page, Mode::Insert, Mode::Hints, Mode::Command, Mode::Visual];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Board => "board",
            Mode::Page => "page",
            Mode::Insert => "insert",
            Mode::Hints => "hints",
            Mode::Command => "command",
            Mode::Visual => "visual",
        }
    }

    pub fn named(name: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.name().eq_ignore_ascii_case(name))
    }

    /// Modes where letters type (a label, a line, a page's text): a
    /// binding there is one chord with Ctrl or Alt, or an F key.
    pub fn types(self) -> bool {
        matches!(self, Mode::Insert | Mode::Hints | Mode::Command)
    }
}

/// `board,page` → the modes; an unknown name says which.
pub fn modes(list: &str) -> Result<Vec<Mode>, String> {
    let mut out = Vec::new();
    for name in list.split(',').map(str::trim) {
        let m = Mode::named(name).ok_or_else(|| format!("no mode “{name}” (board, page, insert, hints, command, visual)"))?;
        if !out.contains(&m) {
            out.push(m);
        }
    }
    Ok(out)
}

/// One line of the key file, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// `bind <modes> <keys> <chain>`.
    Bind { modes: Vec<Mode>, keys: Vec<Chord>, target: String },
    /// `unbind <modes> <keys>`.
    Unbind { modes: Vec<Mode>, keys: Vec<Chord> },
    /// `alias <name> <chain>`.
    Alias { name: String, target: String },
}

/// One step of a chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Action(Action),
    /// A command line, aliases already expanded.
    Line(String),
}

/// How deep aliases may use aliases.
const MAX_DEPTH: usize = 8;

/// Splits a chain at its bars; `\|` stays a bar.
pub fn split_chain(chain: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut chars = chain.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                parts.last_mut().expect("never empty").push('|');
                chars.next();
            }
            '|' => parts.push(String::new()),
            c => parts.last_mut().expect("never empty").push(c),
        }
    }
    parts.into_iter().map(|p| p.trim().to_string()).collect()
}

/// A valid alias name: letters, digits, `-` and `_`, starting with a letter.
pub fn alias_name_ok(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_alphabetic()) && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Aliases: a short name for a chain. Names are kept lower case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aliases {
    list: Vec<(String, String)>,
}

impl Aliases {
    /// Adds or replaces `name`. It can't be a command's name or short name.
    pub fn set(&mut self, name: &str, target: &str) -> Result<(), String> {
        let name = name.to_lowercase();
        if !alias_name_ok(&name) {
            return Err(format!("“{name}” can't be an alias name (letters, digits, - and _)"));
        }
        if command::COMMANDS.iter().any(|s| s.name == name || s.aliases.contains(&name.as_str())) {
            return Err(format!("“{name}” is a command already"));
        }
        if target.trim().is_empty() {
            return Err(format!("alias {name} needs a command"));
        }
        match self.list.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = target.trim().to_string(),
            None => self.list.push((name, target.trim().to_string())),
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) {
        let lower = name.to_lowercase();
        self.list.retain(|(n, _)| *n != lower);
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.list.iter().find(|(n, _)| *n == lower).map(|(_, t)| t.as_str())
    }

    /// (name, chain), in the order they were made.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.list.iter().map(|(n, t)| (n.as_str(), t.as_str()))
    }
}

/// Reads a chain into its steps: action names, aliases (what follows an
/// alias's name goes after its chain's last command) and command lines,
/// each checked. Placeholders (`{url}`) are filled when a line runs.
pub fn steps(chain: &str, aliases: &Aliases) -> Result<Vec<Step>, String> {
    let mut out = Vec::new();
    expand(chain, aliases, 0, &mut out)?;
    Ok(out)
}

fn expand(chain: &str, aliases: &Aliases, depth: usize, out: &mut Vec<Step>) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("aliases use each other too deeply (a loop?)".into());
    }
    for part in split_chain(chain) {
        if part.is_empty() {
            return Err(format!("an empty command in “{}”", chain.trim()));
        }
        let (count, line) = command::split_count(&part);
        if let Some(n) = count {
            // A count before a command: its count when it takes one and has
            // none (`3 back` → `back 3`), else it runs that many times.
            let takes = command::parse(line).is_ok_and(|p| p.arg.is_none() && p.spec.arg.kind() == Some(command::Kind::Count));
            let line = if takes { format!("{line} {n}") } else { format!("repeat {n} {}", line.replace('|', r"\|")) };
            expand(&line, aliases, depth + 1, out)?;
            continue;
        }
        let line = line.trim_start_matches(':').trim();
        let (name, rest) = line.split_once(char::is_whitespace).map_or((line, ""), |(n, r)| (n, r.trim()));
        if rest.is_empty()
            && let Some(a) = Action::named(name)
        {
            out.push(Step::Action(a));
        } else if let Some(target) = aliases.get(name) {
            let joined = if rest.is_empty() { target.to_string() } else { format!("{target} {rest}") };
            expand(&joined, aliases, depth + 1, out)?;
        } else {
            match command::parse(line) {
                Ok(p) if matches!(p.spec.cmd, command::Cmd::Later | command::Cmd::Repeat) => {
                    check_held(&p, aliases, depth)?;
                    out.push(Step::Line(line.to_string()));
                }
                Ok(_) => out.push(Step::Line(line.to_string())),
                // A placeholder stands for what a number or choice will be.
                Err(command::Error::BadArgument(..)) if PLACEHOLDERS.iter().any(|p| line.contains(&format!("{{{p}}}"))) => out.push(Step::Line(line.to_string())),
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    Ok(())
}

/// `:later` and `:repeat`: a number in range, then a chain that reads.
/// A repeat can't hold another (a count before a command is one), so a
/// line never runs more than [`command::MAX_TIMES`] times over.
fn check_held(p: &command::Parsed, aliases: &Aliases, depth: usize) -> Result<(), String> {
    let repeat = p.spec.cmd == command::Cmd::Repeat;
    let name = p.spec.name;
    let (n, inner) = command::split_times(p.arg.as_deref().unwrap_or(""), if repeat { "times" } else { "milliseconds" })
        .map_err(|e| format!("{name}: {e}"))?;
    if repeat && !(1..=command::MAX_TIMES as u64).contains(&n) {
        return Err(format!("repeat: 1 to {} times", command::MAX_TIMES));
    }
    if !repeat && n > command::MAX_LATER_MS {
        return Err(format!("later: at most {} ms (a day)", command::MAX_LATER_MS));
    }
    let mut held = Vec::new();
    expand(&inner, aliases, depth + 1, &mut held).map_err(|e| format!("{name}: {e}"))?;
    let repeats = |s: &Step| matches!(s, Step::Line(l) if command::parse(l).is_ok_and(|p| p.spec.cmd == command::Cmd::Repeat));
    if repeat && held.iter().any(repeats) {
        return Err("repeat: can't hold another repeat or a count".into());
    }
    Ok(())
}

/// What `:later` or `:repeat` runs, read: the number and the chain's
/// steps (checked when the line was read, so an error here is rare).
pub fn held(line: &str, aliases: &Aliases) -> Result<(u64, Vec<Step>), String> {
    let p = command::parse(line).map_err(|e| e.to_string())?;
    let (n, inner) = command::split_times(p.arg.as_deref().unwrap_or(""), "times").map_err(|e| format!("{}: {e}", p.spec.name))?;
    Ok((n, steps(&inner, aliases)?))
}

/// The placeholders a line may hold.
pub const PLACEHOLDERS: &[&str] = &["url", "title", "hint-url"];

/// Fills `{url}`, `{title}` and `{hint-url}` in `text` from `value`; a
/// placeholder with no value (no page here, no hint) says so. Other
/// braces stay as they are.
pub fn fill(text: &str, value: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}').map(|end| &after[..end]).filter(|n| PLACEHOLDERS.contains(n)) {
            Some(name) => {
                let v = value(name).ok_or_else(|| match name {
                    "hint-url" => "{hint-url} needs a link chosen by hints (:hint)".to_string(),
                    _ => format!("{{{name}}} needs a page here"),
                })?;
                out.push_str(&v);
                rest = &after[name.len() + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// Splits `:run`'s words: spaces part them; `'…'` is taken as it is;
/// `"…"` too, but `\"` and `\\` inside are a quote and a backslash; a
/// backslash outside quotes takes the next character as it is. No shell
/// ever sees them.
pub fn words(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if let Some(w) = word.take() {
                    out.push(w);
                }
            }
            '\'' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => w.push(c),
                        None => return Err("a ' isn't closed".into()),
                    }
                }
            }
            '"' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => w.push(c),
                            Some(c) => {
                                w.push('\\');
                                w.push(c);
                            }
                            None => return Err("a \" isn't closed".into()),
                        },
                        Some(c) => w.push(c),
                        None => return Err("a \" isn't closed".into()),
                    }
                }
            }
            '\\' => {
                let w = word.get_or_insert_with(String::new);
                match chars.next() {
                    Some(c) => w.push(c),
                    None => w.push('\\'),
                }
            }
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    if let Some(w) = word {
        out.push(w);
    }
    Ok(out)
}

/// Every mode's table, the lines user bindings run, and the aliases.
#[derive(Debug, Clone)]
pub struct Keys {
    pub board: Bindings<Action>,
    pub page: Bindings<Action>,
    pub insert: Bindings<Action>,
    pub hints: Bindings<Action>,
    pub command: Bindings<Action>,
    pub visual: Bindings<Action>,
    /// What [`Action::Line`] `n` runs, and the chain as written.
    pub lines: Vec<(Vec<Step>, String)>,
    pub aliases: Aliases,
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            board: keys::board(),
            page: keys::page(),
            insert: keys::insert(),
            hints: Bindings::new(),
            command: Bindings::new(),
            visual: keys::visual(),
            lines: Vec::new(),
            aliases: Aliases::default(),
        }
    }
}

/// The most lines a set of keys holds.
const MAX_LINES: usize = u16::MAX as usize;

impl Keys {
    pub fn table(&self, m: Mode) -> &Bindings<Action> {
        match m {
            Mode::Board => &self.board,
            Mode::Page => &self.page,
            Mode::Insert => &self.insert,
            Mode::Hints => &self.hints,
            Mode::Command => &self.command,
            Mode::Visual => &self.visual,
        }
    }

    fn table_mut(&mut self, m: Mode) -> &mut Bindings<Action> {
        match m {
            Mode::Board => &mut self.board,
            Mode::Page => &mut self.page,
            Mode::Insert => &mut self.insert,
            Mode::Hints => &mut self.hints,
            Mode::Command => &mut self.command,
            Mode::Visual => &mut self.visual,
        }
    }

    /// The defaults with `entries` applied: aliases first (a binding may
    /// use one made further down), then bindings in order. A bad entry is
    /// skipped and said with its line number; the rest still apply.
    pub fn build(entries: &[(usize, Entry)]) -> (Keys, Vec<(usize, String)>) {
        let mut k = Keys::default();
        let mut errors = Vec::new();
        // Every alias first, so they may use each other in any order; then
        // each is checked, and a bad one goes.
        let mut made = Vec::new();
        for (n, e) in entries {
            if let Entry::Alias { name, target } = e {
                match k.aliases.set(name, target) {
                    Ok(()) => made.push((*n, name.to_lowercase())),
                    Err(msg) => errors.push((*n, msg)),
                }
            }
        }
        let bad: Vec<(usize, String, String)> = made
            .iter()
            .filter_map(|(n, name)| {
                let target = k.aliases.get(name)?;
                steps(target, &k.aliases).err().map(|msg| (*n, name.clone(), msg))
            })
            .collect();
        for (n, name, msg) in bad {
            k.aliases.remove(&name);
            errors.push((n, format!("alias {name}: {msg}")));
        }
        for (n, e) in entries.iter().filter(|(_, e)| !matches!(e, Entry::Alias { .. })) {
            if let Err(msg) = k.apply(e) {
                errors.push((*n, msg));
            }
        }
        errors.sort_by_key(|(n, _)| *n);
        (k, errors)
    }

    /// Applies one entry; what it did, for a toast.
    pub fn apply(&mut self, e: &Entry) -> Result<String, String> {
        match e {
            Entry::Alias { name, target } => {
                steps(target, &self.aliases_with(name, target)?)?;
                self.aliases.set(name, target)?;
                Ok(format!("Alias {} → {}", name.to_lowercase(), target.trim()))
            }
            Entry::Unbind { modes, keys: seq } => {
                let gone: usize = modes.iter().map(|m| self.table_mut(*m).unbind(seq)).sum();
                if gone == 0 {
                    return Err(format!("{} isn't bound in {}", keys::display(seq), mode_list(modes)));
                }
                Ok(format!("Unbound {} in {}", keys::display(seq), mode_list(modes)))
            }
            Entry::Bind { modes, keys: seq, target } => {
                let mut chain = steps(target, &self.aliases)?;
                // An action that does nothing here but is also a command's
                // name (`tidy` in a page) runs as that command.
                for s in &mut chain {
                    if let Step::Action(a) = *s
                        && modes.iter().any(|m| !self.works_in(*m, a))
                        && let Some(name) = a.name().filter(|n| command::COMMANDS.iter().any(|c| c.name == *n))
                    {
                        *s = Step::Line(name.to_string());
                    }
                }
                for m in modes {
                    check_keys(*m, seq)?;
                    for s in &chain {
                        if let Step::Action(a) = s
                            && !self.works_in(*m, *a)
                        {
                            return Err(format!("{} isn't a {} key", a.name().unwrap_or("that"), m.name()));
                        }
                    }
                }
                let action = match chain.as_slice() {
                    [Step::Action(a)] => *a,
                    _ => {
                        if self.lines.len() >= MAX_LINES {
                            return Err("too many bindings".into());
                        }
                        self.lines.push((chain, target.trim().to_string()));
                        Action::Line((self.lines.len() - 1) as u16)
                    }
                };
                for m in modes {
                    self.table_mut(*m).bind_chords(seq.clone(), action);
                }
                Ok(format!("Bound {} in {}", keys::display(seq), mode_list(modes)))
            }
        }
    }

    /// The aliases as they would be with `name` set (so an alias checks
    /// against itself: a loop is found).
    fn aliases_with(&self, name: &str, target: &str) -> Result<Aliases, String> {
        let mut a = self.aliases.clone();
        a.set(name, target)?;
        Ok(a)
    }

    /// Whether action `a` does something in mode `m`: board and page
    /// actions where the default tables have them, page actions in Insert
    /// too. Hints and the command line run command lines only.
    fn works_in(&self, m: Mode, a: Action) -> bool {
        match m {
            Mode::Board => keys::board().has(a),
            Mode::Page => keys::page().has(a),
            Mode::Insert => keys::insert().has(a) || keys::page().has(a),
            Mode::Visual => keys::visual().has(a) || keys::page().has(a),
            Mode::Hints | Mode::Command => false,
        }
    }

    /// What a binding does, for the sheet: an action says it, a line is shown.
    pub fn about(&self, a: Action) -> String {
        match a {
            Action::Line(n) => self.lines.get(n as usize).map_or_else(String::new, |(_, t)| format!(":{t}")),
            a => a.about(),
        }
    }
}

fn mode_list(modes: &[Mode]) -> String {
    modes.iter().map(|m| m.name()).collect::<Vec<_>>().join(", ")
}

/// Keys a binding in `m` may use.
fn check_keys(m: Mode, seq: &[Chord]) -> Result<(), String> {
    if m.types() {
        let ok = match seq {
            [c] => (c.ctrl || c.alt || matches!(c.key, Key::F(_))) && c.key != Key::Esc,
            _ => false,
        };
        if !ok {
            return Err(format!("in {} a binding is one key with Ctrl or Alt, or an F key ({} types there)", m.name(), keys::display(seq)));
        }
    }
    if seq.first().is_some_and(|c| *c == Chord::plain(Key::Esc)) {
        return Err("Esc can't start a binding".into());
    }
    Ok(())
}

/// A sequence as the `?` sheet writes keys: `gg`, `Ctrl+D`, `Ctrl+W then h`.
pub fn sheet_keys(seq: &[Chord]) -> String {
    if seq.iter().all(|c| c.is_bare() && matches!(c.key, Key::Char(_))) {
        return seq.iter().map(|c| if let Key::Char(ch) = c.key { ch } else { ' ' }).collect();
    }
    seq.iter().map(sheet_chord).collect::<Vec<_>>().join(" then ")
}

fn sheet_chord(c: &Chord) -> String {
    let key = match c.key {
        Key::Char(ch) if c.is_bare() => ch.to_string(),
        Key::Char(ch) => ch.to_uppercase().to_string(),
        Key::Esc => "Esc".into(),
        Key::Enter => "Enter".into(),
        Key::Tab => "Tab".into(),
        Key::Space => "Space".into(),
        Key::Backspace => "Backspace".into(),
        Key::Delete => "Delete".into(),
        Key::Insert => "Insert".into(),
        Key::Left => "Left".into(),
        Key::Right => "Right".into(),
        Key::Up => "Up".into(),
        Key::Down => "Down".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PgUp".into(),
        Key::PageDown => "PgDn".into(),
        Key::F(n) => format!("F{n}"),
        Key::Menu => "Menu".into(),
    };
    format!("{}{}{}{key}", if c.ctrl { "Ctrl+" } else { "" }, if c.alt { "Alt+" } else { "" }, if c.shift { "Shift+" } else { "" })
}

/// Alternatives a row shows at most.
const MAX_ALTS: usize = 4;

/// A table as the `?` sheet's rows (keys, what they do), one per action,
/// in [`Action::rank`] order. Keys for the same thing are alternatives
/// (`" · "`), shortest first, without Shift twins of keys already listed
/// (`Shift+Enter` beside `Enter`). The 52 mark letters and Alt+digits
/// collapse to a range (`m then a…z`).
pub fn sheet_rows(table: &Bindings<Action>, about: &dyn Fn(Action) -> String) -> Vec<(String, String)> {
    // Families: the marks by case, and the card numbers.
    #[derive(PartialEq, Eq, Clone, Copy, Hash)]
    enum Group {
        One(Action),
        SetMark(bool),
        GoMark(bool),
        Nth,
    }
    let group = |a: Action| match a {
        Action::SetMark(c) => Group::SetMark(c.is_ascii_lowercase()),
        Action::GoMark(c) if c != '\'' => Group::GoMark(c.is_ascii_lowercase()),
        Action::NthCard(n) if n < 9 => Group::Nth,
        a => Group::One(a),
    };
    let mut groups: Vec<(Group, Action, Vec<Vec<Chord>>)> = Vec::new();
    for (seq, a) in table.iter() {
        let g = group(a);
        match groups.iter_mut().find(|(x, ..)| *x == g) {
            Some((_, _, seqs)) => seqs.push(seq.to_vec()),
            None => groups.push((g, a, vec![seq.to_vec()])),
        }
    }
    groups.sort_by_key(|(_, a, seqs)| (a.rank(), seqs.iter().map(|s| sheet_keys(s)).min()));
    groups
        .into_iter()
        .map(|(g, a, mut seqs)| {
            seqs.sort_by_key(|s| (s.len(), sheet_keys(s).chars().count(), sheet_keys(s)));
            let keys = match g {
                Group::One(_) => alternatives(&seqs),
                _ => range(&seqs).unwrap_or_else(|| alternatives(&seqs)),
            };
            (keys, about(a))
        })
        .collect()
}

fn alternatives(seqs: &[Vec<Chord>]) -> String {
    let unshifted = |s: &Vec<Chord>| {
        let mut t = s.clone();
        let last = t.last_mut()?;
        if !last.shift {
            return None;
        }
        last.shift = false;
        Some(t)
    };
    // Ctrl+w then Ctrl+h is Ctrl+w then h's twin (Ctrl kept held).
    let held = |s: &Vec<Chord>| {
        if s.len() < 2 {
            return None;
        }
        let mut t = s.clone();
        let last = t.last_mut()?;
        if !last.ctrl {
            return None;
        }
        last.ctrl = false;
        Some(t)
    };
    let shown: Vec<String> = seqs
        .iter()
        .filter(|s| unshifted(s).is_none_or(|t| !seqs.contains(&t)))
        .filter(|s| held(s).is_none_or(|t| !seqs.contains(&t)))
        .map(|s| sheet_keys(s))
        .collect();
    let more = shown.len() > MAX_ALTS;
    let mut out = shown.into_iter().take(MAX_ALTS).collect::<Vec<_>>().join(" · ");
    if more {
        out.push_str(" …");
    }
    out
}

/// Sequences that differ only in their last key, a character each:
/// `m then a…z`, `Alt+1…8`. Several starts (`'` and `` ` ``) give one
/// range each, as alternatives.
fn range(seqs: &[Vec<Chord>]) -> Option<String> {
    let mut starts: Vec<(Vec<Chord>, bool, bool, Vec<char>)> = Vec::new();
    for s in seqs {
        let (last, head) = s.split_last()?;
        let Key::Char(c) = last.key else { return None };
        let key = (head.to_vec(), last.ctrl, last.alt);
        match starts.iter_mut().find(|(h, ct, al, _)| (h, *ct, *al) == (&key.0, key.1, key.2)) {
            Some((.., cs)) => cs.push(if last.shift { c.to_ascii_uppercase() } else { c }),
            None => starts.push((key.0, key.1, key.2, vec![if last.shift { c.to_ascii_uppercase() } else { c }])),
        }
    }
    let parts: Vec<String> = starts
        .into_iter()
        .map(|(head, ctrl, alt, mut cs)| {
            cs.sort_unstable();
            let span = match (cs.first(), cs.last()) {
                (Some(a), Some(b)) if a != b => format!("{a}…{b}"),
                (Some(a), _) => a.to_string(),
                _ => String::new(),
            };
            let mods = format!("{}{}", if ctrl { "Ctrl+" } else { "" }, if alt { "Alt+" } else { "" });
            if head.is_empty() { format!("{mods}{span}") } else { format!("{} then {mods}{span}", sheet_keys(&head)) }
        })
        .collect();
    Some(parts.join(" · "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::Dir;

    fn seq(s: &str) -> Vec<Chord> {
        keys::parse(s).unwrap()
    }

    fn bind(modes: &str, k: &str, target: &str) -> Entry {
        Entry::Bind { modes: super::modes(modes).unwrap(), keys: seq(k), target: target.into() }
    }

    #[test]
    fn chains_split_at_bars() {
        assert_eq!(split_chain("close | reopen"), vec!["close", "reopen"]);
        assert_eq!(split_chain(r"open a\|b"), vec!["open a|b"]);
        assert_eq!(split_chain("x||y"), vec!["x", "", "y"]);
    }

    #[test]
    fn steps_read_actions_aliases_and_lines() {
        let mut a = Aliases::default();
        a.set("Arch", "open https://web.archive.org/web/{url}").unwrap();
        a.set("twice", "card-next | card-next").unwrap();
        assert_eq!(steps("scroll-down", &a).unwrap(), vec![Step::Action(Action::Scroll(Dir::Down))]);
        assert_eq!(steps(":tidy | close", &a).unwrap(), vec![Step::Action(Action::Tidy), Step::Action(Action::Close)]);
        assert_eq!(steps("arch", &a).unwrap(), vec![Step::Line("open https://web.archive.org/web/{url}".into())]);
        // What follows an alias goes after its last command.
        assert_eq!(steps("twice 3", &a).unwrap(), vec![Step::Line("card-next".into()), Step::Line("card-next 3".into())]);
        assert_eq!(steps("zoom {x}", &a).unwrap_err(), "zoom: “{x}” isn't <in|out|reset>");
        assert!(steps("zoom {url}", &a).is_ok(), "a placeholder may stand for a choice");
        assert!(steps("nonsense", &a).unwrap_err().contains("No command"));
        assert!(steps("close |", &a).unwrap_err().contains("empty"));
        // An action name with words after it is a command line.
        assert_eq!(steps("back 2", &a).unwrap(), vec![Step::Line("back 2".into())]);
    }

    #[test]
    fn counts_later_and_repeat() {
        let mut a = Aliases::default();
        a.set("twice", "card-next | card-next").unwrap();
        // A count gives a command its count, else repeats it.
        assert_eq!(steps("3 back", &a).unwrap(), vec![Step::Line("back 3".into())]);
        assert_eq!(steps(":2card-next", &a).unwrap(), vec![Step::Line("card-next 2".into())]);
        assert_eq!(steps("3 tidy", &a).unwrap(), vec![Step::Line("repeat 3 tidy".into())]);
        assert_eq!(steps("3 back 2", &a).unwrap(), vec![Step::Line("repeat 3 back 2".into())]);
        assert_eq!(steps("2 twice | close", &a).unwrap(), vec![Step::Line("repeat 2 twice".into()), Step::Action(Action::Close)]);
        assert_eq!(steps("2 scroll-down", &a).unwrap(), vec![Step::Line("repeat 2 scroll-down".into())]);
        // What later and repeat hold is checked when the line is read.
        assert!(steps("later 500 reload", &a).is_ok());
        assert!(steps(r"repeat 2 back \| tidy", &a).is_ok());
        assert_eq!(steps("later soon reload", &a).unwrap_err(), "later: “soon” isn't a number of milliseconds");
        assert_eq!(steps("repeat 3", &a).unwrap_err(), "repeat: a command goes after the times");
        assert_eq!(steps("repeat 0 tidy", &a).unwrap_err(), "repeat: 1 to 100 times");
        assert!(steps("repeat 3 nonsense", &a).unwrap_err().starts_with("repeat: No command"));
        assert!(steps("later 99999999999 tidy", &a).unwrap_err().contains("a day"));
        assert!(steps("repeat 3 repeat 3 tidy", &a).unwrap_err().contains("another repeat"));
        assert!(steps("repeat 3 3 tidy", &a).unwrap_err().contains("another repeat"));
        assert!(steps("100 back", &a).is_ok(), "a count a command takes isn't a repeat");
        assert!(steps("repeat 5 later 1000 reload", &a).is_ok());
        // A bar that was `\|` reaches the step as a plain bar.
        assert_eq!(steps(r"repeat 2 back \| tidy", &a).unwrap(), vec![Step::Line("repeat 2 back | tidy".into())]);
        let (n, held) = super::held("repeat 2 back | tidy", &a).unwrap();
        assert_eq!((n, held), (2, vec![Step::Action(Action::Back), Step::Action(Action::Tidy)]));
    }

    #[test]
    fn aliases_check_names_and_loops() {
        let mut a = Aliases::default();
        assert!(a.set("open", "tidy").unwrap_err().contains("command"));
        assert!(a.set("q", "tidy").unwrap_err().contains("command"), "a short name too");
        assert!(a.set("1x", "tidy").is_err());
        assert!(a.set("x", " ").is_err());
        let alias = |n: &str, t: &str| Entry::Alias { name: n.into(), target: t.into() };
        let (k, errors) = Keys::build(&[
            (1, alias("loop-a", "loop-b")),
            (2, alias("loop-b", "loop-a")),
            // Used before it is made: fine.
            (3, alias("two", "one | one")),
            (4, alias("one", "card-next")),
        ]);
        assert_eq!(errors.iter().map(|(n, _)| *n).collect::<Vec<_>>(), vec![1, 2], "{errors:?}");
        assert!(errors[0].1.contains("deeply"));
        assert_eq!(k.aliases.get("loop-a"), None);
        assert_eq!(steps("two", &k.aliases).unwrap().len(), 2);
    }

    #[test]
    fn placeholders_fill() {
        let v = |n: &str| match n {
            "url" => Some("https://e.com/a b".to_string()),
            "title" => Some("T".to_string()),
            _ => None,
        };
        assert_eq!(fill("open x/{url}", &v).unwrap(), "open x/https://e.com/a b");
        assert_eq!(fill("{title}{title} {nope} {", &v).unwrap(), "TT {nope} {");
        assert!(fill("{hint-url}", &v).unwrap_err().contains(":hint"));
        assert!(fill("{url}", &|_| None).unwrap_err().contains("page"));
    }

    #[test]
    fn words_split_like_a_command_line_without_a_shell() {
        assert_eq!(words("mpv  --fs {url}").unwrap(), vec!["mpv", "--fs", "{url}"]);
        assert_eq!(words(r#"a 'b c' "d \"e\" \n" f\ g"#).unwrap(), vec!["a", "b c", r#"d "e" \n"#, "f g"]);
        assert_eq!(words("x''y \"\"").unwrap(), vec!["xy", ""]);
        assert_eq!(words("$(rm -rf) ; |").unwrap(), vec!["$(rm", "-rf)", ";", "|"]);
        assert!(words("'open").is_err());
        assert!(words("\"open").is_err());
        assert!(words("   ").unwrap().is_empty());
    }

    #[test]
    fn build_binds_unbinds_and_reports_lines() {
        let entries = vec![
            (1, bind("page", "x", "close")),
            (2, bind("board,page", "gs", "open https://e.com/{url} | tidy")),
            (3, Entry::Unbind { modes: super::modes("page").unwrap(), keys: seq("d") }),
            (4, bind("page", "<C-p>", "top")),
            (5, bind("board", "X", "top")),
            (6, bind("insert", "x", "tidy")),
            (7, bind("insert", "<C-e>", "scroll-down")),
            (8, bind("hints", "<A-r>", "run mpv {url}")),
            (9, bind("hints", "<C-x>", "close")),
            (10, Entry::Unbind { modes: super::modes("board").unwrap(), keys: seq("zz") }),
            (11, Entry::Unbind { modes: super::modes("page").unwrap(), keys: seq(";") }),
            (12, bind("page", "<Esc>x", "tidy")),
        ];
        let (k, errors) = Keys::build(&entries);
        let lines: Vec<usize> = errors.iter().map(|(n, _)| *n).collect();
        assert_eq!(lines, vec![5, 6, 10, 12], "{errors:?}");
        let get = |m: Mode, s: &str| match k.table(m).lookup(&seq(s)) {
            keys::Lookup::Exact(a) | keys::Lookup::ExactAndPrefix(a) => Some(a),
            _ => None,
        };
        assert_eq!(get(Mode::Page, "x"), Some(Action::Close));
        assert_eq!(get(Mode::Page, "d"), None);
        assert_eq!(get(Mode::Board, "d"), Some(Action::Close), "only page lost d");
        assert_eq!(get(Mode::Page, "<C-p>"), Some(Action::Top));
        assert_eq!(get(Mode::Insert, "<C-e>"), Some(Action::Scroll(Dir::Down)));
        let line = get(Mode::Board, "gs").unwrap();
        assert_eq!(get(Mode::Page, "gs"), Some(line), "one line for both modes");
        let Action::Line(n) = line else { panic!("{line:?}") };
        assert_eq!(k.lines[n as usize].0, vec![Step::Line("open https://e.com/{url}".into()), Step::Line("tidy".into())]);
        assert_eq!(k.about(line), ":open https://e.com/{url} | tidy");
        assert!(matches!(get(Mode::Hints, "<A-r>"), Some(Action::Line(_))));
        // close does nothing as a hints key, but it is a command too.
        let Some(Action::Line(n)) = get(Mode::Hints, "<C-x>") else { panic!() };
        assert_eq!(k.lines[n as usize].0, vec![Step::Line("close".into())]);
        // `;` went with every hint kind under it; g still starts gg.
        assert_eq!(get(Mode::Page, ";y"), None);
        assert_eq!(k.table(Mode::Page).lookup(&seq(";")), keys::Lookup::None);
        assert_eq!(get(Mode::Page, "gg"), Some(Action::Top));
    }

    #[test]
    fn sheet_keys_read_like_the_sheet() {
        assert_eq!(sheet_keys(&seq("gg")), "gg");
        assert_eq!(sheet_keys(&seq("<C-d>")), "Ctrl+D");
        assert_eq!(sheet_keys(&seq("<C-S-z>")), "Ctrl+Shift+Z");
        assert_eq!(sheet_keys(&seq("<C-w>h")), "Ctrl+W then h");
        assert_eq!(sheet_keys(&seq("<A-Left>")), "Alt+Left");
        assert_eq!(sheet_keys(&seq("<Space>")), "Space");
    }

    #[test]
    fn sheet_rows_group_and_collapse() {
        let k = Keys::default();
        let rows = sheet_rows(&k.page, &|a| k.about(a));
        let row = |about: &str| rows.iter().find(|(_, d)| d == about).map(|(k, _)| k.clone());
        assert_eq!(row("To the top").as_deref(), Some("gg"));
        assert_eq!(row("Close the card (or the selection)").as_deref(), Some("d · Ctrl+W then c · Ctrl+W then q"), "Ctrl+W alone closes nothing; Ctrl+W then Ctrl+Q is a twin");
        assert_eq!(row("Mark the page's scroll spot").as_deref(), Some("m then a…z"));
        assert_eq!(row("Mark the board spot").as_deref(), Some("m then A…Z"));
        assert_eq!(row("Go to a page mark").as_deref(), Some("' then a…z · ` then a…z"));
        assert_eq!(row("The nth card in reading order").as_deref(), Some("Alt+1…8"));
        assert_eq!(row("The card used before this one").as_deref(), Some("Ctrl+6 · Ctrl+^ · Ctrl+W then p"), "Ctrl+Shift+6 is Ctrl+6's twin");
        // In NAMED order: scrolling before cards.
        let at = |about: &str| rows.iter().position(|(_, d)| d == about).unwrap();
        assert!(at("Scroll down") < at("Next card (reading order)"));
        let b = sheet_rows(&k.board, &|a| k.about(a));
        let brow = |about: &str| b.iter().find(|(_, d)| d == about).map(|(k, _)| k.clone());
        assert_eq!(brow("Type in the card's page").as_deref(), Some("Enter · Space"));
        // What can follow `g`.
        let after = sheet_rows(&k.page.after(&seq("g")), &|a| k.about(a));
        assert!(after.contains(&("g".to_string(), "To the top".to_string())), "{after:?}");
        assert!(after.iter().all(|(keys, _)| keys.chars().count() == 1));
    }
}
