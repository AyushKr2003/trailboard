//! winit keyboard events → toolkit-neutral `web::keys::Key`.

use web::flags;
use web::keys::{vk, Key};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::platform::scancode::PhysicalKeyExtScancode;

pub fn modifiers(m: ModifiersState) -> u32 {
    let mut f = 0;
    if m.shift_key() {
        f |= flags::SHIFT;
    }
    if m.control_key() {
        f |= flags::CONTROL;
    }
    if m.alt_key() {
        f |= flags::ALT;
    }
    f
}

/// The key as the current layout means it, expressed as the US-layout `KeyCode`
/// carrying that meaning (e.g. "l" on any layout → `KeyL`). Falls back to the
/// physical key for keys without a logical mapping.
pub fn shortcut_code(ev: &KeyEvent) -> Option<KeyCode> {
    use winit::keyboard::Key as L;
    let logical = ev.key_without_modifiers();
    let from_logical = match &logical {
        L::Character(s) => {
            let mut chars = s.chars();
            match (chars.next().map(|c| c.to_ascii_lowercase()), chars.next()) {
                (Some(c @ 'a'..='z'), None) => Some(letter_code(c)),
                (Some(c @ '0'..='9'), None) => Some(digit_code(c)),
                _ => None,
            }
        }
        L::Named(n) => named_code(*n),
        _ => None,
    };
    from_logical.or(match ev.physical_key {
        PhysicalKey::Code(c) => Some(c),
        PhysicalKey::Unidentified(_) => None,
    })
}

fn letter_code(c: char) -> KeyCode {
    use KeyCode::*;
    const LETTERS: [KeyCode; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ,
        KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    LETTERS[(c as u8 - b'a') as usize]
}

fn digit_code(c: char) -> KeyCode {
    use KeyCode::*;
    const DIGITS: [KeyCode; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    DIGITS[(c as u8 - b'0') as usize]
}

fn named_code(n: winit::keyboard::NamedKey) -> Option<KeyCode> {
    use winit::keyboard::NamedKey as N;
    use KeyCode::*;
    Some(match n {
        N::Escape => Escape,
        N::Enter => Enter,
        N::Tab => Tab,
        N::Backspace => Backspace,
        N::Delete => Delete,
        N::Insert => Insert,
        N::Space => Space,
        N::ArrowLeft => ArrowLeft,
        N::ArrowRight => ArrowRight,
        N::ArrowUp => ArrowUp,
        N::ArrowDown => ArrowDown,
        N::Home => Home,
        N::End => End,
        N::PageUp => PageUp,
        N::PageDown => PageDown,
        N::Shift => ShiftLeft,
        N::Control => ControlLeft,
        N::Alt => AltLeft,
        N::F1 => F1,
        N::F2 => F2,
        N::F3 => F3,
        N::F4 => F4,
        N::F5 => F5,
        N::F6 => F6,
        N::F7 => F7,
        N::F8 => F8,
        N::F9 => F9,
        N::F10 => F10,
        N::F11 => F11,
        N::F12 => F12,
        _ => return None,
    })
}

/// A key press or release, independent of where it came from (winit or a test script).
#[derive(Debug, Clone)]
pub struct KeyIn {
    pub pressed: bool,
    /// Layout-aware key for shortcuts (see [`shortcut_code`]).
    pub code: Option<KeyCode>,
    /// The logical key if it is a named key (Enter, Escape, arrows…).
    pub named: Option<NamedKey>,
    /// Text the key produces, as the platform reports it.
    pub text: Option<String>,
    /// X11/XKB keycode, 0 if unknown.
    pub native: i32,
}

impl KeyIn {
    pub fn from_winit(ev: &KeyEvent) -> KeyIn {
        KeyIn {
            pressed: ev.state == ElementState::Pressed,
            code: shortcut_code(ev),
            named: match &ev.logical_key {
                winit::keyboard::Key::Named(n) => Some(*n),
                _ => None,
            },
            text: ev.text.as_ref().map(|t| t.to_string()),
            // X11/XKB keycode = evdev scancode + 8.
            native: ev.physical_key.to_scancode().map_or(0, |s| s as i32 + 8),
        }
    }

    /// A US-layout key from a script: a single character (`a`, `7`, `/`), or a
    /// name (`enter`, `esc`, `tab`, `backspace`, `delete`, `left`, `right`, `up`,
    /// `down`, `home`, `end`, `space`, `f1`…). `shift` upper-cases letters.
    pub fn from_name(name: &str, pressed: bool, shift: bool) -> Option<KeyIn> {
        use winit::keyboard::NamedKey as N;
        use KeyCode::*;
        let lower = name.to_ascii_lowercase();
        let named = |code: KeyCode, n: NamedKey, text: Option<&str>| (code, Some(n), text.map(str::to_string));
        let (code, named, text) = match lower.as_str() {
            "enter" | "return" => named(Enter, N::Enter, Some("\r")),
            "esc" | "escape" => named(Escape, N::Escape, None),
            "tab" => named(Tab, N::Tab, Some("\t")),
            "backspace" => named(Backspace, N::Backspace, None),
            "delete" | "del" => named(Delete, N::Delete, None),
            "left" => named(ArrowLeft, N::ArrowLeft, None),
            "right" => named(ArrowRight, N::ArrowRight, None),
            "up" => named(ArrowUp, N::ArrowUp, None),
            "down" => named(ArrowDown, N::ArrowDown, None),
            "home" => named(Home, N::Home, None),
            "end" => named(End, N::End, None),
            "pageup" => named(PageUp, N::PageUp, None),
            "pagedown" => named(PageDown, N::PageDown, None),
            "space" => named(Space, N::Space, Some(" ")),
            f if f.len() >= 2 && f.starts_with('f') && f[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) => {
                const FS: [(KeyCode, NamedKey); 12] = [
                    (F1, N::F1), (F2, N::F2), (F3, N::F3), (F4, N::F4), (F5, N::F5), (F6, N::F6),
                    (F7, N::F7), (F8, N::F8), (F9, N::F9), (F10, N::F10), (F11, N::F11), (F12, N::F12),
                ];
                let (c, n) = FS[f[1..].parse::<usize>().unwrap() - 1];
                named(c, n, None)
            }
            _ => {
                let mut chars = name.chars();
                let (Some(c), None) = (chars.next(), chars.next()) else { return None };
                let code = char_code(c.to_ascii_lowercase())?;
                let c = if shift { c.to_ascii_uppercase() } else { c };
                (code, None, Some(c.to_string()))
            }
        };
        Some(KeyIn {
            pressed,
            code: Some(code),
            named,
            text: if pressed { text } else { None },
            native: PhysicalKey::Code(code).to_scancode().map_or(0, |s| s as i32 + 8),
        })
    }

    /// The event Chromium gets for this key.
    pub fn to_web(&self, mods: ModifiersState) -> Key {
        // Ctrl/Alt combos are shortcuts, not text (AltGr arrives without ctrl+alt both set on most layouts).
        let text = if self.pressed && !(mods.control_key() || mods.alt_key()) {
            self.text
                .as_ref()
                .map(|t| t.chars().filter(|c| !c.is_control() || *c == '\r' || *c == '\t').collect::<String>())
                .filter(|t| !t.is_empty())
        } else {
            None
        };
        Key {
            pressed: self.pressed,
            vk: vk_code(self.code.unwrap_or(KeyCode::Fn)),
            native: self.native,
            modifiers: modifiers(mods),
            text,
        }
    }
}

/// Parse a script key combo like `ctrl+shift+z` into modifiers and the key name.
pub fn parse_combo(combo: &str) -> Option<(ModifiersState, &str)> {
    let mut mods = ModifiersState::empty();
    let mut parts: Vec<&str> = combo.split('+').collect();
    // "ctrl++" means Ctrl and the plus key.
    if combo.ends_with("++") {
        parts.truncate(parts.len() - 2);
        parts.push("+");
    }
    let key = parts.pop().filter(|k| !k.is_empty())?;
    for m in parts {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= ModifiersState::CONTROL,
            "shift" => mods |= ModifiersState::SHIFT,
            "alt" => mods |= ModifiersState::ALT,
            _ => return None,
        }
    }
    Some((mods, key))
}

/// US-layout `KeyCode` for a printable ASCII character (lowercase letters).
fn char_code(c: char) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match c {
        'a'..='z' => letter_code(c),
        '0'..='9' => digit_code(c),
        ' ' => Space,
        '-' => Minus,
        '=' | '+' => Equal,
        ',' => Comma,
        '.' => Period,
        '/' => Slash,
        ';' => Semicolon,
        '\'' => Quote,
        '`' => Backquote,
        '[' => BracketLeft,
        ']' => BracketRight,
        '\\' => Backslash,
        _ => return None,
    })
}

fn vk_code(c: KeyCode) -> i32 {
    use KeyCode::*;
    match c {
        Backspace => vk::BACK,
        Tab => vk::TAB,
        Enter | NumpadEnter => vk::RETURN,
        ShiftLeft | ShiftRight => vk::SHIFT,
        ControlLeft | ControlRight => vk::CONTROL,
        AltLeft | AltRight => vk::MENU,
        Escape => vk::ESCAPE,
        Space => vk::SPACE,
        PageUp => vk::PRIOR,
        PageDown => vk::NEXT,
        End => vk::END,
        Home => vk::HOME,
        ArrowLeft => vk::LEFT,
        ArrowUp => vk::UP,
        ArrowRight => vk::RIGHT,
        ArrowDown => vk::DOWN,
        Insert => vk::INSERT,
        Delete => vk::DELETE,
        Digit0 | Numpad0 => 0x30,
        Digit1 | Numpad1 => 0x31,
        Digit2 | Numpad2 => 0x32,
        Digit3 | Numpad3 => 0x33,
        Digit4 | Numpad4 => 0x34,
        Digit5 | Numpad5 => 0x35,
        Digit6 | Numpad6 => 0x36,
        Digit7 | Numpad7 => 0x37,
        Digit8 | Numpad8 => 0x38,
        Digit9 | Numpad9 => 0x39,
        KeyA => 0x41,
        KeyB => 0x42,
        KeyC => 0x43,
        KeyD => 0x44,
        KeyE => 0x45,
        KeyF => 0x46,
        KeyG => 0x47,
        KeyH => 0x48,
        KeyI => 0x49,
        KeyJ => 0x4A,
        KeyK => 0x4B,
        KeyL => 0x4C,
        KeyM => 0x4D,
        KeyN => 0x4E,
        KeyO => 0x4F,
        KeyP => 0x50,
        KeyQ => 0x51,
        KeyR => 0x52,
        KeyS => 0x53,
        KeyT => 0x54,
        KeyU => 0x55,
        KeyV => 0x56,
        KeyW => 0x57,
        KeyX => 0x58,
        KeyY => 0x59,
        KeyZ => 0x5A,
        F1 => vk::F1,
        F2 => vk::F1 + 1,
        F3 => vk::F1 + 2,
        F4 => vk::F1 + 3,
        F5 => vk::F1 + 4,
        F6 => vk::F1 + 5,
        F7 => vk::F1 + 6,
        F8 => vk::F1 + 7,
        F9 => vk::F1 + 8,
        F10 => vk::F1 + 9,
        F11 => vk::F1 + 10,
        F12 => vk::F1 + 11,
        Semicolon => vk::OEM_1,
        Equal => vk::OEM_PLUS,
        Comma => vk::OEM_COMMA,
        Minus => vk::OEM_MINUS,
        Period => vk::OEM_PERIOD,
        Slash => vk::OEM_2,
        Backquote => vk::OEM_3,
        BracketLeft => vk::OEM_4,
        Backslash => vk::OEM_5,
        BracketRight => vk::OEM_6,
        Quote => vk::OEM_7,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos_parse() {
        assert_eq!(parse_combo("ctrl+shift+z"), Some((ModifiersState::CONTROL | ModifiersState::SHIFT, "z")));
        assert_eq!(parse_combo("a"), Some((ModifiersState::empty(), "a")));
        assert_eq!(parse_combo("ctrl++"), Some((ModifiersState::CONTROL, "+")));
        assert_eq!(parse_combo("hyper+a"), None);
        assert_eq!(parse_combo("ctrl+"), None);
    }

    #[test]
    fn named_keys_map_to_vk() {
        let k = KeyIn::from_name("c", true, false).unwrap();
        let web = k.to_web(ModifiersState::CONTROL);
        assert_eq!(web.vk, 0x43);
        assert_eq!(web.text, None, "ctrl combos carry no text");
        assert_eq!(web.native, 46 + 8, "evdev KEY_C + 8");
        let k = KeyIn::from_name("q", true, true).unwrap();
        assert_eq!(k.to_web(ModifiersState::SHIFT).text.as_deref(), Some("Q"));
        let k = KeyIn::from_name("enter", true, false).unwrap();
        assert_eq!(k.named, Some(NamedKey::Enter));
        assert_eq!(k.to_web(ModifiersState::empty()).vk, vk::RETURN);
        assert!(KeyIn::from_name("f13", true, false).is_none());
        assert!(KeyIn::from_name("é", true, false).is_none());
    }
}
