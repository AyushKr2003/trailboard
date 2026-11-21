//! A text field's text out to an editor and back (`Ctrl+e` in Insert mode,
//! M12 step 15). Our own design.
//!
//! [`read`] runs in the focused frame: it remembers the focused field under
//! a token and answers with a console message starting with [`PREFIX`]
//! carrying the field's text. [`write`] puts new text into the field the
//! token names, as typing would (so the page's own handlers and undo see
//! it), and answers too.

/// Prefix of a field report.
pub const PREFIX: &str = "\u{1}trailboard-field\u{1}";

/// One answer from a frame about the field under `token`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// The field's text, as [`read`] found it.
    Text { token: u64, text: String },
    /// [`read`] found no text field with the focus.
    NoField { token: u64 },
    /// [`write`] put the text in.
    Put { token: u64 },
    /// [`write`] found the field gone (the page moved on).
    Gone { token: u64 },
}

/// Reads a report's body (after [`PREFIX`]): `token\nwhat[\ntext]`.
pub fn parse(body: &str) -> Option<Report> {
    let (token, rest) = body.split_once('\n')?;
    let token: u64 = token.parse().ok()?;
    let (what, text) = rest.split_once('\n').unwrap_or((rest, ""));
    Some(match what {
        "text" => Report::Text { token, text: text.to_string() },
        "none" => Report::NoField { token },
        "put" => Report::Put { token },
        "gone" => Report::Gone { token },
        _ => return None,
    })
}

const COMMON: &str = r#"
  const P = '\u0001trailboard-field\u0001';
  const typed = /^(text|search|email|url|tel|number)$/i;
"#;

/// Remembers the focused text field under `token` and sends its text.
pub fn read(token: u64) -> String {
    format!(
        r#"(() => {{{COMMON}
  let e = document.activeElement;
  while (e && e.shadowRoot && e.shadowRoot.activeElement) e = e.shadowRoot.activeElement;
  const ok = !!e && (e.isContentEditable || e.tagName === 'TEXTAREA' || (e.tagName === 'INPUT' && typed.test(e.type || 'text')));
  if (!ok) {{ console.log(P + '{token}\nnone'); return; }}
  Object.defineProperty(window, '__tbField', {{ value: {{ el: e, token: {token} }}, configurable: true }});
  const text = e.isContentEditable ? e.innerText : e.value;
  console.log(P + '{token}\ntext\n' + text);
}})();"#
    )
}

/// Replaces the text of the field remembered under `token` with `text`.
pub fn write(token: u64, text: &str) -> String {
    let text = crate::js_string(text);
    format!(
        r#"(() => {{{COMMON}
  const f = window.__tbField;
  if (!f || f.token !== {token} || !f.el.isConnected) {{ console.log(P + '{token}\ngone'); return; }}
  const e = f.el, text = {text};
  e.focus();
  let done = false;
  try {{
    if (e.isContentEditable) getSelection().selectAllChildren(e); else e.select();
    done = document.execCommand('insertText', false, text) && (e.isContentEditable || e.value === text);
  }} catch (x) {{}}
  if (!done) {{
    if (e.isContentEditable) e.innerText = text;
    else {{
      const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e), 'value');
      if (d && d.set) d.set.call(e, text); else e.value = text;
    }}
    e.dispatchEvent(new Event('input', {{ bubbles: true }}));
    e.dispatchEvent(new Event('change', {{ bubbles: true }}));
  }}
  console.log(P + '{token}\nput');
}})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_parse() {
        assert_eq!(parse("7\ntext\nline 1\nline 2"), Some(Report::Text { token: 7, text: "line 1\nline 2".into() }));
        assert_eq!(parse("7\ntext\n"), Some(Report::Text { token: 7, text: String::new() }));
        assert_eq!(parse("3\nnone"), Some(Report::NoField { token: 3 }));
        assert_eq!(parse("3\nput"), Some(Report::Put { token: 3 }));
        assert_eq!(parse("3\ngone"), Some(Report::Gone { token: 3 }));
        assert_eq!(parse("x\nput"), None);
        assert_eq!(parse("3\nwhat"), None);
    }

    #[test]
    fn scripts_carry_the_token_and_quote_the_text() {
        assert!(COMMON.contains(&PREFIX.replace('\u{1}', "\\u0001")));
        assert!(read(42).contains("token: 42"));
        let w = write(42, "say \"hi\"\n");
        assert!(w.contains("f.token !== 42"), "{w}");
        assert!(w.contains(r#"text = "say \"hi\"\n";"#), "{w}");
    }
}
