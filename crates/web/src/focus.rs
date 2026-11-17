//! Where a page's keyboard focus is, for Insert mode (M12). Our own design.
//!
//! [`SCRIPT`] runs in every frame of web and app pages as they load. It
//! answers with a console message starting with [`PREFIX`] when focus moves
//! into or out of something you type in (text inputs, text areas, selects,
//! editable content, ARIA text boxes; inside open shadow roots too), and
//! after every mouse press, so a click on plain page text reports "not a
//! field". A frame whose focus sits in a child frame says nothing: the child
//! frame reports for itself.
//!
//! Each message says whether the focus came with a user gesture
//! (`navigator.userActivation`), so a field a page focuses on its own as it
//! loads never switches modes. [`FOCUS_FIRST_FIELD`] focuses the first
//! visible text field and reports it like a user's focus, or reports that
//! there is none.

/// Prefix of a focus report.
pub const PREFIX: &str = "\u{1}trailboard-focus\u{1}";

/// A frame's focus, from one report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// Focus is in a field (`user`: with a user gesture), or not.
    Field { editable: bool, user: bool },
    /// [`FOCUS_FIRST_FIELD`] found no field to focus.
    NoField,
}

/// Reads a report's body (after [`PREFIX`]).
pub fn parse(body: &str) -> Option<Report> {
    match body {
        "none" => Some(Report::NoField),
        "0" => Some(Report::Field { editable: false, user: false }),
        "0u" => Some(Report::Field { editable: false, user: true }),
        "1" => Some(Report::Field { editable: true, user: false }),
        "1u" => Some(Report::Field { editable: true, user: true }),
        _ => None,
    }
}

const COMMON: &str = r#"
  const P = '\u0001trailboard-focus\u0001';
  const typed = /^(text|search|email|url|tel|password|number|date|datetime-local|month|week|time)$/i;
  const deep = () => {
    let e = document.activeElement;
    while (e && e.shadowRoot && e.shadowRoot.activeElement) e = e.shadowRoot.activeElement;
    return e;
  };
  const editable = e => !!e && (e.isContentEditable || e.tagName === 'TEXTAREA' || e.tagName === 'SELECT' ||
    (e.tagName === 'INPUT' && typed.test(e.type || 'text')) ||
    /^(textbox|combobox|searchbox)$/.test((e.getAttribute && e.getAttribute('role')) || ''));
"#;

/// Installs the focus reports in a frame (once per document).
pub fn script() -> String {
    format!(
        r#"(() => {{
  if (window.__tbFocus) return;
  Object.defineProperty(window, '__tbFocus', {{ value: true }});
  {COMMON}
  let last = '';
  const report = force => setTimeout(() => {{
    const e = deep();
    if (e && (e.tagName === 'IFRAME' || e.tagName === 'FRAME')) return;
    const s = (editable(e) && document.hasFocus() ? '1' : '0');
    if (!force && s === last) return;
    last = s;
    const u = navigator.userActivation && navigator.userActivation.isActive ? 'u' : '';
    console.log(P + s + u);
  }}, 0);
  addEventListener('focusin', () => report(false), true);
  addEventListener('focusout', () => report(false), true);
  addEventListener('mousedown', () => report(true), true);
}})();"#
    )
}

/// Focuses the first visible, enabled text field in the frame (`gi`) and
/// reports it as the user's focus; reports `none` if there is none.
pub fn focus_first_field() -> String {
    format!(
        r#"(() => {{
  {COMMON}
  const sel = 'input, textarea, [contenteditable=""], [contenteditable=true], [role=textbox], [role=searchbox]';
  for (const e of document.querySelectorAll(sel)) {{
    if (e.disabled || e.readOnly || e.closest('[aria-hidden=true], [inert]')) continue;
    if (e.tagName === 'INPUT' && !typed.test(e.type || 'text')) continue;
    const r = e.getBoundingClientRect();
    if (r.width < 2 || r.height < 2 || r.bottom < 0 || r.top > innerHeight || r.right < 0 || r.left > innerWidth) continue;
    e.focus();
    if (editable(deep())) {{ console.log(P + '1u'); return; }}
  }}
  console.log(P + 'none');
}})();"#
    )
}

/// Takes the focus out of whatever field has it (Esc in Insert mode), so the
/// page keeps the keys but no caret; inside shadow roots too.
pub const BLUR: &str = r#"(() => {
  let e = document.activeElement, chain = [];
  while (e) { chain.push(e); e = e.shadowRoot && e.shadowRoot.activeElement; }
  for (const x of chain.reverse()) if (x.blur) x.blur();
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_parse() {
        assert_eq!(parse("1u"), Some(Report::Field { editable: true, user: true }));
        assert_eq!(parse("0"), Some(Report::Field { editable: false, user: false }));
        assert_eq!(parse("none"), Some(Report::NoField));
        assert_eq!(parse("2"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn scripts_are_whole() {
        // The shared part landed inside each function and the braces balance.
        for s in [script(), focus_first_field()] {
            assert!(s.contains("trailboard-focus") && s.contains("const deep"));
            assert_eq!(s.matches('{').count(), s.matches('}').count());
        }
    }
}
