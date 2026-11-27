//! The note editor page: our own HTML (`note.html`) with the note's text and
//! the theme's colours filled in, loaded into a note card as a `data:` URL.

use crate::ui::theme::{mix, Rgb, Theme};

/// Size of a new sticky note's content, world units.
pub const STICKY_W: f32 = 280.0;
pub const STICKY_H: f32 = 240.0;

const EDITOR: &str = include_str!("note.html");

pub(crate) fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// The theme's colours as the editor's CSS variables, a JSON object. A
/// note is a card, so it is written on the card level in the chrome's text
/// strengths. A sticky note (`sticky`, its colour name) is written on its
/// coloured paper, everything in shades of its ink.
fn colors(th: &Theme, sticky: Option<&str>) -> String {
    let vars = match sticky {
        None => [
            ("bg", th.card(false)),
            ("fg", th.text()),
            ("dim", th.text2()),
            ("faint", th.text3()),
            ("link", th.accent_text()),
            ("accent", th.accent()),
            ("sel", th.accent_soft()),
            ("code", th.panel()),
            ("rule", th.line()),
            ("rulestrong", th.line_strong()),
        ],
        Some(c) => {
            // Ink on coloured paper: links and boxes in the ink, not the accent,
            // which may not read on it.
            let (paper, ink) = (th.sticky_paper(c), th.sticky_ink(c));
            [
                ("bg", paper),
                ("fg", ink),
                ("dim", mix(ink, paper, 0.3)),
                ("faint", mix(ink, paper, 0.45)),
                ("link", ink),
                ("accent", ink),
                ("sel", mix(paper, ink, 0.22)),
                ("code", mix(paper, ink, 0.08)),
                ("rule", mix(paper, ink, 0.18)),
                ("rulestrong", mix(paper, ink, 0.35)),
            ]
        }
    };
    let body: Vec<String> = vars
        .iter()
        .map(|(k, c)| format!("\"{k}\":\"{}\"", hex(*c)))
        .collect();
    // Hyprland's window rounding, which code blocks follow (capped small);
    // whether the note is a sticky note (tighter, for its small paper).
    format!(
        "{{{},\"radius\":\"{}px\",\"sticky\":{}}}",
        body.join(","),
        th.rounding.max(0.0),
        sticky.is_some()
    )
}

/// The editor page for a note holding `text`, a sticky note in colour `sticky`.
pub fn page_url(text: &str, th: &Theme, sticky: Option<&str>) -> String {
    // The text goes in last, so nothing in it is taken for a placeholder.
    let html = EDITOR
        .replacen("__PREFIX__", &web::js_string(web::NOTE_MESSAGE), 1)
        .replacen("__OPEN__", &web::js_string(web::NOTE_OPEN), 1)
        .replacen("__THEME__", &colors(th, sticky), 1)
        .replacen("__TEXT__", &web::js_string(text), 1);
    format!("data:text/html;charset=utf-8,{}", percent_encode(&html))
}

/// Script that recolours a live note editor (a sticky note in colour `sticky`).
pub fn theme_script(th: &Theme, sticky: Option<&str>) -> String {
    format!("window.tbTheme&&tbTheme({})", colors(th, sticky))
}

/// Script that opens a live note editor for writing (Enter on the board).
pub const EDIT_SCRIPT: &str = "window.tbEdit&&tbEdit()";

/// Script that replaces a live note editor's text.
pub fn set_text_script(text: &str) -> String {
    format!("window.tbSetText&&tbSetText({})", web::js_string(text))
}

/// Every byte but ASCII letters, digits and `-._~` as `%XX`, so the URL is
/// already in the form Chromium keeps (it is compared with the URL loaded).
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(s: &str) -> String {
        let b = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn page_holds_the_text_and_theme() {
        let th = Theme::default();
        let url = page_url("# Hi </script> __THEME__ é", &th, None);
        let body = url.strip_prefix("data:text/html;charset=utf-8,").unwrap();
        assert!(body.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b)));
        let html = decode(body);
        assert!(html.contains(r##"let text = "# Hi \u003c/script\u003e __THEME__ é";"##));
        assert!(html.contains(&format!("window.tbTheme({});", colors(&th, None))));
        assert!(!html.contains("__PREFIX__") && !html.contains("__OPEN__") && !html.contains("__TEXT__"));
        assert!(html.contains(r#"const PREFIX = "\u0001trailboard-note\u0001", OPEN = "\u0001trailboard-open\u0001";"#));
    }

    #[test]
    fn colors_are_css_hex() {
        let c = colors(&Theme::default(), None);
        assert!(c.starts_with("{\"bg\":\"#") && c.ends_with("\"sticky\":false}"));
        assert_eq!(c.matches('#').count(), 10);
    }

    #[test]
    fn sticky_notes_are_written_on_their_colour() {
        let th = Theme::default();
        let paper = hex(th.sticky_paper("yellow"));
        let c = colors(&th, Some("yellow"));
        assert!(c.starts_with(&format!("{{\"bg\":\"{paper}\"")), "{c}");
        assert_eq!(c.matches('#').count(), 10);
        assert!(c.ends_with("\"sticky\":true}"));
        assert_ne!(colors(&th, Some("green")), c);
        assert!(theme_script(&th, Some("yellow")).contains(&paper));
    }
}
