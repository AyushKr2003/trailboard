//! Pure rules for the history index (`storage::pagetext`): which pages are
//! kept, how much of their text, how a typed query becomes a full-text query,
//! and which letters of a result's title to highlight. Our own design.

/// At most this many bytes of a page's text are kept (cut at a char boundary).
pub const MAX_TEXT: usize = 64 * 1024;
/// Titles longer than this are cut.
pub const MAX_TITLE: usize = 500;
/// A query uses at most this many words.
const MAX_WORDS: usize = 8;

/// Whether a page at `url` belongs in history: web pages and local files.
/// Error pages (`chrome-error:`), `data:`, `about:` and the like do not.
pub fn keeps(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else { return false };
    matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https" | "file") && rest.len() > 2
}

/// `text` cut to at most `max` bytes, at a char boundary, without trailing space.
pub fn clamp(text: &str, max: usize) -> &str {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].trim_end()
}

/// The words of a query: runs of letters and digits, lowercased.
pub fn words(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(MAX_WORDS)
        .map(str::to_lowercase)
        .collect()
}

/// An FTS5 query finding pages that contain every word of `query`, the last
/// ones (and all others) as prefixes, so results come while typing. `None` when
/// the query has no words. Words are quoted, so nothing in them is syntax.
pub fn fts_query(query: &str) -> Option<String> {
    let w = words(query);
    if w.is_empty() {
        return None;
    }
    Some(w.iter().map(|w| format!("\"{}\"*", w.replace('"', "\"\""))).collect::<Vec<_>>().join(" "))
}

/// Char indices in `text` to highlight for `query`: for each query word, the
/// letters of every word in `text` that starts with it (case ignored).
pub fn highlight(query: &str, text: &str) -> Vec<usize> {
    let qs = words(query);
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_alphanumeric() {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect::<String>().to_lowercase();
        // Lowercasing can change lengths (rare); then count by the original word.
        let best = qs.iter().filter(|q| word.starts_with(q.as_str())).map(|q| q.chars().count()).max();
        if let Some(n) = best {
            out.extend(start..(start + n).min(i));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_web_and_files_only() {
        assert!(keeps("https://example.com/a"));
        assert!(keeps("HTTP://example.com"));
        assert!(keeps("file:///home/me/notes.html"));
        assert!(!keeps("data:text/html,hi"));
        assert!(!keeps("about:blank"));
        assert!(!keeps("chrome-error://chromewebdata/"));
        assert!(!keeps("devtools://x"));
        assert!(!keeps("https:"));
        assert!(!keeps("nonsense"));
    }

    #[test]
    fn clamp_cuts_at_char_boundary() {
        assert_eq!(clamp("hello", 10), "hello");
        assert_eq!(clamp("hello world", 6), "hello");
        // "é" is two bytes: cutting inside it backs off.
        assert_eq!(clamp("aé", 2), "a");
        assert_eq!(clamp("", 3), "");
    }

    #[test]
    fn query_words_are_quoted_prefixes() {
        assert_eq!(fts_query("Rust  wgpu"), Some("\"rust\"* \"wgpu\"*".into()));
        assert_eq!(fts_query("c++ AND (x) NEAR"), Some("\"c\"* \"and\"* \"x\"* \"near\"*".into()));
        assert_eq!(fts_query("\"*^:-"), None);
        assert_eq!(fts_query("   "), None);
        assert_eq!(words("Ünïcode café"), vec!["ünïcode", "café"]);
        assert_eq!(words("a b c d e f g h i j").len(), MAX_WORDS);
    }

    #[test]
    fn highlight_marks_word_prefixes() {
        assert_eq!(highlight("rus", "Learn Rust"), vec![6, 7, 8]);
        assert_eq!(highlight("le ru", "Learn Rust"), vec![0, 1, 6, 7]);
        // Inside a word is not a match.
        assert_eq!(highlight("ust", "Learn Rust"), Vec::<usize>::new());
        // Every word that starts with it.
        assert_eq!(highlight("a", "a b ab"), vec![0, 4]);
        // The longer of two query words wins.
        assert_eq!(highlight("r rus", "Rust"), vec![0, 1, 2]);
        assert_eq!(highlight("", "Rust"), Vec::<usize>::new());
    }
}
