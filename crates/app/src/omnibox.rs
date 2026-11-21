//! The address/search box. Opens over the board; Enter loads into the focused
//! card or a new one.

use scene::settings::SearchEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Load into the currently focused card.
    Focused,
    /// Open a new card.
    NewCard,
    /// Open beside the current card (Page mode's `O`).
    Beside,
    /// Open beside the current card, staying where you are (`xo`).
    Behind,
    /// Rename the current space (the text is a name, not an address).
    RenameSpace,
    /// Name the trail through this card (a name too).
    NameTrail(scene::NodeId),
    /// The Ctrl+K palette: the text searches cards, spaces and commands (`palette`).
    Palette,
    /// Find in this card's page (Ctrl+F): the text is searched as you type.
    Find(scene::NodeId),
    /// The history view (Ctrl+H): the text filters visited pages (`palette::history_rows`).
    History,
    /// The downloads view (Ctrl+J): the text filters downloads (`palette::download_rows`).
    Downloads,
    /// The sites where ads are allowed (`palette::allowed_rows`).
    AllowedSites,
    /// Where to save the download Chromium is waiting about (a path; *Save
    /// downloads to: Ask each time*). Closing the field cancels the download.
    SaveAs(u32),
    /// The space's background photo (`palette::photo_rows`): the text filters
    /// the pictures offered, or is a picture file's path.
    Photo,
    /// The answer to a page's `prompt` (`App::dialogs`); closing the field
    /// cancels it.
    Prompt(u64),
    /// `:messages` (`palette::message_rows`): the text filters them.
    Messages,
    /// The path for a page's file chooser (`App::choosing`); closing the
    /// field cancels it. Tab completes.
    Files(u64),
}

impl Target {
    /// The field has a list of results under it (the palette, history, downloads or allowed sites).
    pub fn is_list(&self) -> bool {
        matches!(
            self,
            Target::Palette | Target::History | Target::Downloads | Target::AllowedSites | Target::Photo | Target::Messages
        )
    }
}

#[derive(Debug)]
pub struct Omnibox {
    pub text: String,
    /// Caret position as a char index.
    pub caret: usize,
    pub target: Target,
    /// All text selected (typing replaces it), as after Ctrl+L.
    pub select_all: bool,
    /// Palette only: the highlighted result row and the first row shown.
    pub sel: usize,
    pub scroll: usize,
    /// Text the last word or line delete took (Ctrl+W, Ctrl+U, Ctrl+K,
    /// Alt+D); Ctrl+Y puts it back.
    pub killed: String,
    /// Command line: Up / Down walking earlier lines.
    pub walk: Option<scene::command::Walk>,
    /// Command line: while Tab walks the list, the text the list is for
    /// (the field shows the row Tab is on).
    pub list_for: Option<String>,
}

impl Omnibox {
    pub fn new(initial: &str, target: Target) -> Self {
        Self { caret: initial.chars().count(), text: initial.to_string(), target, select_all: !initial.is_empty(), sel: 0, scroll: 0, killed: String::new(), walk: None, list_for: None }
    }

    /// The text the list under the field is for: what was typed, while Tab
    /// walks the list.
    pub fn query(&self) -> &str {
        self.list_for.as_deref().unwrap_or(&self.text)
    }

    /// The field holds `text`, caret at its end.
    pub fn set(&mut self, text: &str) {
        self.text = text.to_string();
        self.caret = text.chars().count();
        self.select_all = false;
    }

    fn chars(&self) -> Vec<char> {
        self.text.chars().collect()
    }

    /// Where the word before the caret starts (readline's Alt+B): back over
    /// spaces and punctuation, then over the word's letters and digits.
    pub fn word_start(&self) -> usize {
        let cs = self.chars();
        let mut i = self.caret.min(cs.len());
        while i > 0 && !cs[i - 1].is_alphanumeric() {
            i -= 1;
        }
        while i > 0 && cs[i - 1].is_alphanumeric() {
            i -= 1;
        }
        i
    }

    /// Where the word after the caret ends (Alt+F).
    pub fn word_end(&self) -> usize {
        let cs = self.chars();
        let mut i = self.caret.min(cs.len());
        while i < cs.len() && !cs[i].is_alphanumeric() {
            i += 1;
        }
        while i < cs.len() && cs[i].is_alphanumeric() {
            i += 1;
        }
        i
    }

    /// Where the space-separated word before the caret starts (Ctrl+W, as
    /// in a shell: `open https://a.com/x` loses the whole address).
    fn big_word_start(&self) -> usize {
        let cs = self.chars();
        let mut i = self.caret.min(cs.len());
        while i > 0 && cs[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !cs[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    /// Removes chars `from..to` and keeps them for Ctrl+Y; the caret goes to `from`.
    fn kill(&mut self, from: usize, to: usize) {
        self.select_all = false;
        if from >= to {
            return;
        }
        let (a, b) = (self.byte_at(from), self.byte_at(to));
        self.killed = self.text[a..b].to_string();
        self.text.replace_range(a..b, "");
        self.caret = from;
    }

    /// Ctrl+W: the space-separated word before the caret.
    pub fn kill_big_word_back(&mut self) {
        self.select_all = false;
        self.kill(self.big_word_start(), self.caret);
    }

    /// Alt+Backspace, Ctrl+Backspace: the word before the caret.
    pub fn kill_word_back(&mut self) {
        self.select_all = false;
        self.kill(self.word_start(), self.caret);
    }

    /// Alt+D, Ctrl+Delete: the word after the caret.
    pub fn kill_word_forward(&mut self) {
        self.select_all = false;
        self.kill(self.caret, self.word_end());
    }

    /// Ctrl+U: everything before the caret (a command line keeps its `:`).
    pub fn kill_to_start(&mut self) {
        self.select_all = false;
        let keep = usize::from(self.text.starts_with(':') && self.caret > 0);
        self.kill(keep, self.caret);
    }

    /// Ctrl+K: everything after the caret.
    pub fn kill_to_end(&mut self) {
        self.select_all = false;
        self.kill(self.caret, self.text.chars().count());
    }

    /// Ctrl+Y: the text deleted last, at the caret.
    pub fn yank(&mut self) {
        self.select_all = false;
        let k = self.killed.clone();
        self.insert(&k);
    }

    /// Alt+B, Ctrl+Left.
    pub fn word_left(&mut self) {
        self.select_all = false;
        self.caret = self.word_start();
    }

    /// Alt+F, Ctrl+Right.
    pub fn word_right(&mut self) {
        self.select_all = false;
        self.caret = self.word_end();
    }

    fn byte_at(&self, char_idx: usize) -> usize {
        self.text.char_indices().nth(char_idx).map_or(self.text.len(), |(b, _)| b)
    }

    pub fn insert(&mut self, s: &str) {
        if self.select_all {
            self.text.clear();
            self.caret = 0;
            self.select_all = false;
        }
        let b = self.byte_at(self.caret);
        self.text.insert_str(b, s);
        self.caret += s.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.select_all {
            self.text.clear();
            self.caret = 0;
            self.select_all = false;
        } else if self.caret > 0 {
            let b = self.byte_at(self.caret - 1);
            self.text.remove(b);
            self.caret -= 1;
        }
    }

    pub fn delete(&mut self) {
        if self.select_all {
            self.backspace();
        } else if self.caret < self.text.chars().count() {
            let b = self.byte_at(self.caret);
            self.text.remove(b);
        }
    }

    pub fn left(&mut self) {
        self.select_all = false;
        self.caret = self.caret.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.select_all = false;
        self.caret = (self.caret + 1).min(self.text.chars().count());
    }

    pub fn home(&mut self) {
        self.select_all = false;
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.select_all = false;
        self.caret = self.text.chars().count();
    }

    /// Text before the caret (for measuring caret x).
    pub fn before_caret(&self) -> &str {
        &self.text[..self.byte_at(self.caret)]
    }
}

/// `engine`'s results page for `query` (no keywords: the text is the query).
pub fn search_url(query: &str, engine: SearchEngine) -> String {
    engine.url(&scene::search::encode_query(query.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing() {
        let mut o = Omnibox::new("abc", Target::Focused);
        o.insert("x"); // replaces selection
        assert_eq!(o.text, "x");
        o.insert("yz");
        o.left();
        o.backspace();
        assert_eq!(o.text, "xz");
        o.home();
        o.delete();
        assert_eq!(o.text, "z");
    }

    #[test]
    fn readline_keys() {
        let mut o = Omnibox::new("", Target::Palette);
        o.set(":open https://a.com/x-y now");
        o.word_left();
        assert_eq!(o.before_caret(), ":open https://a.com/x-y ");
        o.word_left();
        assert_eq!(o.before_caret(), ":open https://a.com/x-");
        o.word_right();
        assert_eq!(o.before_caret(), ":open https://a.com/x-y");
        o.kill_big_word_back();
        assert_eq!(o.text, ":open  now", "Ctrl+W takes the whole address");
        o.yank();
        assert_eq!(o.text, ":open https://a.com/x-y now", "Ctrl+Y puts it back");
        o.home();
        o.right();
        o.kill_word_forward();
        assert_eq!(o.text, ": https://a.com/x-y now");
        o.end();
        o.kill_word_back();
        assert_eq!(o.text, ": https://a.com/x-y ");
        o.kill_to_start();
        assert_eq!((o.text.as_str(), o.caret), (":", 1), "Ctrl+U keeps the command line's colon");
        o.set("abc def");
        o.home();
        o.right();
        o.kill_to_end();
        assert_eq!(o.text, "a");
        assert_eq!(o.killed, "bc def");
        // After Ctrl+L (all selected) the keys act on the text, not the selection.
        let mut o = Omnibox::new("one two", Target::Focused);
        o.kill_big_word_back();
        assert_eq!(o.text, "one ");
        let mut o = Omnibox::new("x", Target::Focused);
        o.kill_to_start();
        assert_eq!(o.text, "");
    }
}
