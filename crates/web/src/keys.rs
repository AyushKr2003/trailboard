//! Toolkit-neutral keyboard events translated to CEF `KeyEvent`s.
//!
//! The app maps its windowing toolkit's keys to Windows virtual-key codes
//! (which Chromium uses on every platform) and passes any produced text.

use cef::{KeyEvent, KeyEventType};

#[derive(Debug, Clone)]
pub struct Key {
    pub pressed: bool,
    /// Windows virtual-key code (`VK_*`), 0 if unknown.
    pub vk: i32,
    /// Platform scancode, passed through as `native_key_code`.
    pub native: i32,
    /// `flags::*` bits.
    pub modifiers: u32,
    /// Text produced by this press, if any (sent as CHAR events).
    pub text: Option<String>,
}

impl Key {
    pub fn to_cef(&self) -> Vec<KeyEvent> {
        let base = KeyEvent {
            size: std::mem::size_of::<cef::sys::cef_key_event_t>(),
            modifiers: self.modifiers,
            windows_key_code: self.vk,
            native_key_code: self.native,
            ..Default::default()
        };
        if !self.pressed {
            return vec![KeyEvent { type_: KeyEventType::KEYUP, ..base }];
        }
        let mut out = vec![KeyEvent { type_: KeyEventType::RAWKEYDOWN, ..base }];
        if let Some(text) = &self.text {
            for unit in text.encode_utf16() {
                out.push(KeyEvent {
                    type_: KeyEventType::CHAR,
                    windows_key_code: unit as i32,
                    character: unit,
                    unmodified_character: unit,
                    ..base
                });
            }
        }
        out
    }
}

/// Windows virtual-key codes used by Chromium.
pub mod vk {
    pub const BACK: i32 = 0x08;
    pub const TAB: i32 = 0x09;
    pub const RETURN: i32 = 0x0D;
    pub const SHIFT: i32 = 0x10;
    pub const CONTROL: i32 = 0x11;
    pub const MENU: i32 = 0x12; // Alt
    pub const ESCAPE: i32 = 0x1B;
    pub const SPACE: i32 = 0x20;
    pub const PRIOR: i32 = 0x21; // Page Up
    pub const NEXT: i32 = 0x22; // Page Down
    pub const END: i32 = 0x23;
    pub const HOME: i32 = 0x24;
    pub const LEFT: i32 = 0x25;
    pub const UP: i32 = 0x26;
    pub const RIGHT: i32 = 0x27;
    pub const DOWN: i32 = 0x28;
    pub const INSERT: i32 = 0x2D;
    pub const DELETE: i32 = 0x2E;
    /// `'0'..='9'` map to 0x30..0x39 and `'A'..='Z'` to 0x41..0x5A.
    pub const F1: i32 = 0x70;
    pub const OEM_1: i32 = 0xBA; // ;:
    pub const OEM_PLUS: i32 = 0xBB;
    pub const OEM_COMMA: i32 = 0xBC;
    pub const OEM_MINUS: i32 = 0xBD;
    pub const OEM_PERIOD: i32 = 0xBE;
    pub const OEM_2: i32 = 0xBF; // /?
    pub const OEM_3: i32 = 0xC0; // `~
    pub const OEM_4: i32 = 0xDB; // [{
    pub const OEM_5: i32 = 0xDC; // \|
    pub const OEM_6: i32 = 0xDD; // ]}
    pub const OEM_7: i32 = 0xDE; // '"
}
