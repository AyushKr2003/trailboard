//! winit keyboard events → toolkit-neutral `web::keys::Key`.

use scene::keys::{Chord, Key as K};
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
    // A keypad key means what Num Lock makes it (a digit, or Page Up, Home…),
    // which the key without modifiers loses: Num Lock is a modifier.
    let logical = if is_keypad(ev.physical_key) { ev.logical_key.clone() } else { ev.key_without_modifiers() };
    layout_code(&logical, ev.physical_key)
}

fn layout_code(logical: &winit::keyboard::Key, physical: PhysicalKey) -> Option<KeyCode> {
    use winit::keyboard::Key as L;
    let from_logical = match logical {
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
    from_logical.or(match physical {
        PhysicalKey::Code(c) => Some(c),
        PhysicalKey::Unidentified(_) => None,
    })
}

/// A key on the numeric keypad that Num Lock switches (digits and `.`).
fn is_keypad(physical: PhysicalKey) -> bool {
    use KeyCode::*;
    matches!(
        physical,
        PhysicalKey::Code(
            Numpad0 | Numpad1 | Numpad2 | Numpad3 | Numpad4 | Numpad5 | Numpad6 | Numpad7 | Numpad8 | Numpad9 | NumpadDecimal
        )
    )
}

/// The X11/XKB keycode Chromium gets. A keypad key that Num Lock off makes a
/// moving key (Page Up, Home, an arrow…) is sent as that key's own, so pages
/// treat it exactly like the dedicated key on keyboards that have one.
fn native_code(physical: PhysicalKey, named: Option<NamedKey>) -> i32 {
    use NamedKey as N;
    let key = match named {
        Some(n @ (N::PageUp | N::PageDown | N::Home | N::End | N::ArrowLeft | N::ArrowRight | N::ArrowUp | N::ArrowDown | N::Insert | N::Delete))
            if is_keypad(physical) =>
        {
            named_code(n).map_or(physical, PhysicalKey::Code)
        }
        _ => physical,
    };
    // X11/XKB keycode = evdev scancode + 8.
    key.to_scancode().map_or(0, |s| s as i32 + 8)
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
        N::ContextMenu => ContextMenu,
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
        let named = match &ev.logical_key {
            winit::keyboard::Key::Named(n) => Some(*n),
            _ => None,
        };
        KeyIn {
            pressed: ev.state == ElementState::Pressed,
            code: shortcut_code(ev),
            named,
            text: ev.text.as_ref().map(|t| t.to_string()),
            native: native_code(ev.physical_key, named),
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
            "menu" => named(ContextMenu, N::ContextMenu, None),
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

    /// The key as a binding sees it (`scene::keys`). Letters and digits come
    /// from the layout-aware `code` (so h/j/k/l work on any layout, as they
    /// always did); other characters from the text the key typed. `None` for
    /// a modifier alone or a key no binding can name.
    pub fn chord(&self, mods: ModifiersState) -> Option<Chord> {
        use winit::keyboard::NamedKey as N;
        let (ctrl, alt, shift) = (mods.control_key(), mods.alt_key(), mods.shift_key());
        let named = match self.named {
            Some(N::Escape) => Some(K::Esc),
            Some(N::Enter) => Some(K::Enter),
            Some(N::Tab) => Some(K::Tab),
            Some(N::Space) => Some(K::Space),
            Some(N::Backspace) => Some(K::Backspace),
            Some(N::Delete) => Some(K::Delete),
            Some(N::Insert) => Some(K::Insert),
            Some(N::ArrowLeft) => Some(K::Left),
            Some(N::ArrowRight) => Some(K::Right),
            Some(N::ArrowUp) => Some(K::Up),
            Some(N::ArrowDown) => Some(K::Down),
            Some(N::Home) => Some(K::Home),
            Some(N::End) => Some(K::End),
            Some(N::PageUp) => Some(K::PageUp),
            Some(N::PageDown) => Some(K::PageDown),
            Some(N::ContextMenu) => Some(K::Menu),
            Some(N::Shift | N::Control | N::Alt | N::Super | N::Meta | N::Hyper | N::AltGraph | N::CapsLock | N::NumLock) => return None,
            _ => None,
        };
        let key = named
            .or_else(|| match self.code? {
                KeyCode::F1 => Some(K::F(1)),
                KeyCode::F2 => Some(K::F(2)),
                KeyCode::F3 => Some(K::F(3)),
                KeyCode::F4 => Some(K::F(4)),
                KeyCode::F5 => Some(K::F(5)),
                KeyCode::F6 => Some(K::F(6)),
                KeyCode::F7 => Some(K::F(7)),
                KeyCode::F8 => Some(K::F(8)),
                KeyCode::F9 => Some(K::F(9)),
                KeyCode::F10 => Some(K::F(10)),
                KeyCode::F11 => Some(K::F(11)),
                KeyCode::F12 => Some(K::F(12)),
                KeyCode::Space => Some(K::Space),
                _ => None,
            })
            .or_else(|| {
                let letter = (b'a'..=b'z').map(char::from).find(|&c| self.code == Some(letter_code(c)));
                let digit = (b'0'..=b'9').map(char::from).find(|&c| self.code == Some(digit_code(c)));
                match (letter, digit) {
                    (Some(c), _) if shift && !ctrl && !alt => Some(K::Char(c.to_ascii_uppercase())),
                    (Some(c), _) => Some(K::Char(c)),
                    // Shift+digit types a symbol (taken from the text below);
                    // with Ctrl or Alt it stays the digit (`<C-S-6>` is Ctrl+^).
                    (None, Some(c)) if !shift || ctrl || alt => Some(K::Char(c)),
                    _ => None,
                }
            })
            .or_else(|| {
                let mut cs = self.text.as_deref()?.chars().filter(|c| !c.is_control());
                match (cs.next(), cs.next()) {
                    (Some(c), None) => Some(K::Char(c)),
                    _ => None,
                }
            })
            .or_else(|| self.code.and_then(us_char).map(K::Char))?;
        Some(Chord::new(key, ctrl, alt, shift))
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

/// The US-layout character of a punctuation key, for chords whose text the
/// platform doesn't give (Ctrl/Alt combos).
fn us_char(c: KeyCode) -> Option<char> {
    use KeyCode::*;
    Some(match c {
        Minus | NumpadSubtract => '-',
        Equal => '=',
        NumpadAdd => '+',
        Comma => ',',
        Period => '.',
        Slash => '/',
        Semicolon => ';',
        Quote => '\'',
        Backquote => '`',
        BracketLeft => '[',
        BracketRight => ']',
        Backslash => '\\',
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
        ContextMenu => 0x5D, // VK_APPS
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
    fn keypad_follows_num_lock() {
        use winit::keyboard::Key as L;
        let nine = PhysicalKey::Code(KeyCode::Numpad9);
        // Num Lock off: the 9 key is Page Up, for shortcuts and for Chromium,
        // which gets the dedicated Page Up key's keycode (112), not the keypad's (81).
        assert_eq!(layout_code(&L::Named(NamedKey::PageUp), nine), Some(KeyCode::PageUp));
        assert_eq!(native_code(nine, Some(NamedKey::PageUp)), 112);
        assert_eq!(native_code(PhysicalKey::Code(KeyCode::Numpad1), Some(NamedKey::End)), 115);
        assert_eq!(native_code(PhysicalKey::Code(KeyCode::Numpad7), Some(NamedKey::Home)), 110);
        assert_eq!(native_code(PhysicalKey::Code(KeyCode::Numpad3), Some(NamedKey::PageDown)), 117);
        // Num Lock on: a digit, on its own key.
        assert_eq!(layout_code(&L::Character("9".into()), nine), Some(KeyCode::Digit9));
        assert_eq!(native_code(nine, None), 81);
        // Keys that aren't on the keypad keep their own keycode; Enter stays the keypad's.
        assert_eq!(native_code(PhysicalKey::Code(KeyCode::PageUp), Some(NamedKey::PageUp)), 112);
        assert_eq!(native_code(PhysicalKey::Code(KeyCode::NumpadEnter), Some(NamedKey::Enter)), 104);
    }

    #[test]
    fn keys_become_chords() {
        use scene::keys::parse;
        let chord = |combo: &str| {
            let (mods, name) = parse_combo(combo).unwrap();
            KeyIn::from_name(name, true, mods.shift_key()).unwrap().chord(mods)
        };
        let one = |s: &str| parse(s).unwrap().into_iter().next();
        assert_eq!(chord("j"), one("j"));
        assert_eq!(chord("shift+j"), one("J"));
        assert_eq!(chord("ctrl+shift+z"), one("<C-S-z>"));
        assert_eq!(chord("ctrl+z"), one("<C-z>"));
        assert_eq!(chord("5"), one("5"));
        assert_eq!(chord("enter"), one("<Enter>"));
        assert_eq!(chord("shift+left"), one("<S-Left>"));
        assert_eq!(chord("space"), one("<Space>"));
        assert_eq!(chord("["), one("["));
        assert_eq!(KeyIn::from_name("+", true, false).unwrap().chord(ModifiersState::empty()), one("+"));
        assert_eq!(chord("ctrl+-"), one("<C-->"));
        assert_eq!(chord("f2"), one("<F2>"));
        // Digits with Ctrl or Alt stay digits: Alt+1, and Ctrl+^ as Ctrl+Shift+6.
        assert_eq!(chord("alt+1"), one("<A-1>"));
        assert_eq!(chord("ctrl+shift+6"), one("<C-S-6>"));
        // A real Shift+4 types "$": the text wins over the digit.
        let dollar = KeyIn { text: Some("$".into()), ..KeyIn::from_name("4", true, true).unwrap() };
        assert_eq!(dollar.chord(ModifiersState::SHIFT), one("$"));
        // A modifier alone is no chord.
        let shift = KeyIn { named: Some(NamedKey::Shift), code: Some(KeyCode::ShiftLeft), text: None, pressed: true, native: 0 };
        assert_eq!(shift.chord(ModifiersState::SHIFT), None);
    }

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
