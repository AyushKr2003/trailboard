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
}

impl Target {
    /// The field has a list of results under it (the palette, history, downloads or allowed sites).
    pub fn is_list(&self) -> bool {
        matches!(
            self,
            Target::Palette | Target::History | Target::Downloads | Target::AllowedSites | Target::Photo
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
}

impl Omnibox {
    pub fn new(initial: &str, target: Target) -> Self {
        Self { caret: initial.chars().count(), text: initial.to_string(), target, select_all: !initial.is_empty(), sel: 0, scroll: 0 }
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
}
