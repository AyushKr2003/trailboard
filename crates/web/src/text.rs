//! A page's text for the history index. [`SCRIPT`] runs in the page's main
//! frame and answers with one console message: [`PREFIX`], the page's address,
//! a newline, its title, a newline, then its visible text (at most
//! [`MAX_CHARS`] characters). Our own design.

/// Prefix of the console message with a page's text.
pub const PREFIX: &str = "\u{1}trailboard-text\u{1}";

/// The most characters of text one message carries.
pub const MAX_CHARS: usize = 65536;

pub const SCRIPT: &str = r#"(() => {
  const b = document.body;
  const t = b ? (b.innerText || '') : '';
  const title = (document.title || '').replace(/\s+/g, ' ');
  console.log('\u0001trailboard-text\u0001' + location.href + '\n' + title + '\n' + t.slice(0, 65536));
})();"#;

/// Splits a message body (after [`PREFIX`]) into address, title and text.
pub fn parse(body: &str) -> Option<(&str, &str, &str)> {
    let (url, rest) = body.split_once('\n')?;
    let (title, text) = rest.split_once('\n').unwrap_or((rest, ""));
    (!url.is_empty()).then_some((url, title, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_address_title_and_text() {
        assert_eq!(parse("https://a/\nA title\nline 1\nline 2"), Some(("https://a/", "A title", "line 1\nline 2")));
        assert_eq!(parse("https://a/\nOnly title"), Some(("https://a/", "Only title", "")));
        assert_eq!(parse("https://a/"), None);
        assert_eq!(parse("\nt\nx"), None);
    }

    #[test]
    fn script_uses_the_prefix_and_limit() {
        assert!(SCRIPT.contains(&PREFIX.replace('\u{1}', "\\u0001")));
        assert!(SCRIPT.contains(&MAX_CHARS.to_string()));
    }
}
