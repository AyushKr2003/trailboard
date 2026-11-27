//! Caret and Visual mode (M12 step 14): moving a caret through a page's
//! text and selecting with it from the keyboard. Our own design.
//!
//! The page does the moving (`web::caret`, with the browser's own notion
//! of characters, words, lines and paragraphs); this module holds what
//! the app decides: which of the three modes follows a key, and the word
//! each motion is called by in the page script.

/// The three ways the keys move through a page's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// A caret and no selection: the keys move it.
    Caret,
    /// A selection from where it started to the caret, by characters.
    Chars,
    /// The same, grown to whole lines at both ends.
    Lines,
}

impl Mode {
    /// Its name in the page script and the log.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Caret => "caret",
            Mode::Chars => "chars",
            Mode::Lines => "lines",
        }
    }

    pub fn named(name: &str) -> Option<Mode> {
        [Mode::Caret, Mode::Chars, Mode::Lines].into_iter().find(|m| m.name() == name)
    }

    /// What the corner says.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Caret => "Caret · v selects",
            Mode::Chars => "Visual · y copies",
            Mode::Lines => "Visual line · y copies",
        }
    }

    /// `v`: start selecting from the caret, or stop (back to the caret).
    pub fn chars(self) -> Mode {
        match self {
            Mode::Chars => Mode::Caret,
            _ => Mode::Chars,
        }
    }

    /// `V`: select whole lines, or stop.
    pub fn lines(self) -> Mode {
        match self {
            Mode::Lines => Mode::Caret,
            _ => Mode::Lines,
        }
    }

    /// Esc: a selection goes and the caret stays; from the caret, out of
    /// the mode (`None`, back to Page mode).
    pub fn back(self) -> Option<Mode> {
        match self {
            Mode::Caret => None,
            Mode::Chars | Mode::Lines => Some(Mode::Caret),
        }
    }

    pub fn selects(self) -> bool {
        self != Mode::Caret
    }
}

/// Where a key moves the caret (or the moving end of a selection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Motion {
    Left,
    Right,
    Down,
    Up,
    /// The start of the next word (`w`).
    WordNext,
    /// The start of this or the previous word (`b`).
    WordBack,
    /// The end of this or the next word (`e`).
    WordEnd,
    /// The start / end of the line (`0`, `$`).
    LineStart,
    LineEnd,
    /// The start / end of the page's text (`gg`, `G`).
    Top,
    Bottom,
    /// The start of this or the previous paragraph / the end of this or
    /// the next one (`{`, `}`).
    ParaBack,
    ParaNext,
}

impl Motion {
    pub const ALL: [Motion; 13] = [
        Motion::Left,
        Motion::Right,
        Motion::Down,
        Motion::Up,
        Motion::WordNext,
        Motion::WordBack,
        Motion::WordEnd,
        Motion::LineStart,
        Motion::LineEnd,
        Motion::Top,
        Motion::Bottom,
        Motion::ParaBack,
        Motion::ParaNext,
    ];

    /// Its word in the page script (`web::caret`).
    pub fn op(self) -> &'static str {
        match self {
            Motion::Left => "left",
            Motion::Right => "right",
            Motion::Down => "down",
            Motion::Up => "up",
            Motion::WordNext => "word",
            Motion::WordBack => "back",
            Motion::WordEnd => "end",
            Motion::LineStart => "home",
            Motion::LineEnd => "eol",
            Motion::Top => "top",
            Motion::Bottom => "bottom",
            Motion::ParaBack => "para-back",
            Motion::ParaNext => "para",
        }
    }

    /// A count repeats it; the ends of a line or the page are where they
    /// are however often you go.
    pub fn counts(self) -> bool {
        !matches!(self, Motion::LineStart | Motion::LineEnd | Motion::Top | Motion::Bottom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v_and_shift_v_toggle_and_switch() {
        assert_eq!(Mode::Caret.chars(), Mode::Chars);
        assert_eq!(Mode::Chars.chars(), Mode::Caret);
        assert_eq!(Mode::Lines.chars(), Mode::Chars);
        assert_eq!(Mode::Caret.lines(), Mode::Lines);
        assert_eq!(Mode::Lines.lines(), Mode::Caret);
        assert_eq!(Mode::Chars.lines(), Mode::Lines);
    }

    #[test]
    fn esc_drops_the_selection_then_the_mode() {
        assert_eq!(Mode::Chars.back(), Some(Mode::Caret));
        assert_eq!(Mode::Lines.back(), Some(Mode::Caret));
        assert_eq!(Mode::Caret.back(), None);
    }

    #[test]
    fn names_round_trip_and_ops_are_distinct() {
        for m in [Mode::Caret, Mode::Chars, Mode::Lines] {
            assert_eq!(Mode::named(m.name()), Some(m));
        }
        assert_eq!(Mode::named("visual"), None);
        let ops: std::collections::HashSet<_> = Motion::ALL.iter().map(|m| m.op()).collect();
        assert_eq!(ops.len(), Motion::ALL.len());
        assert!(Motion::Down.counts() && !Motion::Bottom.counts());
    }
}
