//! Protocol identity, HID usage codes, Play-mode key-action parsing, and the
//! Programming-mode request-6 encoder.
//!
//! Play-mode [`KeyAction`] uses standard HID modifier bits. Programming-mode
//! types ([`Pedal`], [`ProgramAction`]) are a separate owned macro model that
//! encodes captured/static-cross-checked request-6 payloads. `savant program`
//! writes those mappings. `savant erase` is a separate request-8 write.

use std::fmt;
use std::str::FromStr;

use anyhow::{anyhow, Result};

/// Kinesis vendor ID shared by Play and Program identities.
pub const KINESIS_VID: u16 = 0x05F3;
/// Normal Play-mode product ID.
pub const SAVANT_ELITE_PID: u16 = 0x030C;
/// Programming-mode product ID (from the vendor driver INF).
pub const PROGRAMMING_PID: u16 = 0x0232;

/// USB HID keyboard usage codes.
/// See: https://usb.org/sites/default/files/hut1_4.pdf (Section 10)
/// These constants document the full HID spec even if not all are currently used.
#[allow(dead_code)]
pub mod usb_hid {
    // Modifier keys (byte 0 of keyboard report)
    pub const MOD_LEFT_CTRL: u8 = 0x01;
    pub const MOD_LEFT_SHIFT: u8 = 0x02;
    pub const MOD_LEFT_ALT: u8 = 0x04;
    pub const MOD_LEFT_GUI: u8 = 0x08; // Command on Mac
    pub const MOD_RIGHT_CTRL: u8 = 0x10;
    pub const MOD_RIGHT_SHIFT: u8 = 0x20;
    pub const MOD_RIGHT_ALT: u8 = 0x40;
    pub const MOD_RIGHT_GUI: u8 = 0x80;

    // Common key codes (bytes 2-7 of keyboard report)
    pub const KEY_A: u8 = 0x04;
    pub const KEY_B: u8 = 0x05;
    pub const KEY_C: u8 = 0x06;
    pub const KEY_D: u8 = 0x07;
    pub const KEY_E: u8 = 0x08;
    pub const KEY_F: u8 = 0x09;
    pub const KEY_G: u8 = 0x0A;
    pub const KEY_H: u8 = 0x0B;
    pub const KEY_I: u8 = 0x0C;
    pub const KEY_J: u8 = 0x0D;
    pub const KEY_K: u8 = 0x0E;
    pub const KEY_L: u8 = 0x0F;
    pub const KEY_M: u8 = 0x10;
    pub const KEY_N: u8 = 0x11;
    pub const KEY_O: u8 = 0x12;
    pub const KEY_P: u8 = 0x13;
    pub const KEY_Q: u8 = 0x14;
    pub const KEY_R: u8 = 0x15;
    pub const KEY_S: u8 = 0x16;
    pub const KEY_T: u8 = 0x17;
    pub const KEY_U: u8 = 0x18;
    pub const KEY_V: u8 = 0x19;
    pub const KEY_W: u8 = 0x1A;
    pub const KEY_X: u8 = 0x1B;
    pub const KEY_Y: u8 = 0x1C;
    pub const KEY_Z: u8 = 0x1D;
    pub const KEY_1: u8 = 0x1E;
    pub const KEY_2: u8 = 0x1F;
    pub const KEY_3: u8 = 0x20;
    pub const KEY_4: u8 = 0x21;
    pub const KEY_5: u8 = 0x22;
    pub const KEY_6: u8 = 0x23;
    pub const KEY_7: u8 = 0x24;
    pub const KEY_8: u8 = 0x25;
    pub const KEY_9: u8 = 0x26;
    pub const KEY_0: u8 = 0x27;
    pub const KEY_ENTER: u8 = 0x28;
    pub const KEY_ESC: u8 = 0x29;
    pub const KEY_BACKSPACE: u8 = 0x2A;
    pub const KEY_TAB: u8 = 0x2B;
    pub const KEY_SPACE: u8 = 0x2C;
    pub const KEY_F1: u8 = 0x3A;
    pub const KEY_F2: u8 = 0x3B;
    pub const KEY_F3: u8 = 0x3C;
    pub const KEY_F4: u8 = 0x3D;
    pub const KEY_F5: u8 = 0x3E;
    pub const KEY_F6: u8 = 0x3F;
    pub const KEY_F7: u8 = 0x40;
    pub const KEY_F8: u8 = 0x41;
    pub const KEY_F9: u8 = 0x42;
    pub const KEY_F10: u8 = 0x43;
    pub const KEY_F11: u8 = 0x44;
    pub const KEY_F12: u8 = 0x45;
    pub const KEY_PRINTSCREEN: u8 = 0x46;
    pub const KEY_SCROLLLOCK: u8 = 0x47;
    pub const KEY_PAUSE: u8 = 0x48;
    pub const KEY_INSERT: u8 = 0x49;
    pub const KEY_HOME: u8 = 0x4A;
    pub const KEY_PAGEUP: u8 = 0x4B;
    pub const KEY_DELETE: u8 = 0x4C;
    pub const KEY_END: u8 = 0x4D;
    pub const KEY_PAGEDOWN: u8 = 0x4E;
    pub const KEY_RIGHT: u8 = 0x4F;
    pub const KEY_LEFT: u8 = 0x50;
    pub const KEY_DOWN: u8 = 0x51;
    pub const KEY_UP: u8 = 0x52;
    pub const KEY_NUMLOCK: u8 = 0x53;
    pub const KEY_KEYPAD_DIVIDE: u8 = 0x54;
    pub const KEY_KEYPAD_MULTIPLY: u8 = 0x55;
    pub const KEY_KEYPAD_SUBTRACT: u8 = 0x56;
    pub const KEY_KEYPAD_ADD: u8 = 0x57;
    pub const KEY_KEYPAD_ENTER: u8 = 0x58;
    pub const KEY_KEYPAD_1: u8 = 0x59;
    pub const KEY_KEYPAD_2: u8 = 0x5A;
    pub const KEY_KEYPAD_3: u8 = 0x5B;
    pub const KEY_KEYPAD_4: u8 = 0x5C;
    pub const KEY_KEYPAD_5: u8 = 0x5D;
    pub const KEY_KEYPAD_6: u8 = 0x5E;
    pub const KEY_KEYPAD_7: u8 = 0x5F;
    pub const KEY_KEYPAD_8: u8 = 0x60;
    pub const KEY_KEYPAD_9: u8 = 0x61;
    pub const KEY_KEYPAD_0: u8 = 0x62;
    pub const KEY_KEYPAD_DECIMAL: u8 = 0x63;
    pub const KEY_APPLICATION: u8 = 0x65;
    pub const KEY_F13: u8 = 0x68;
    pub const KEY_F14: u8 = 0x69;
    pub const KEY_F15: u8 = 0x6A;
    pub const KEY_F16: u8 = 0x6B;
    pub const KEY_F17: u8 = 0x6C;
    pub const KEY_F18: u8 = 0x6D;
    pub const KEY_F19: u8 = 0x6E;
    pub const KEY_F20: u8 = 0x6F;
    pub const KEY_F21: u8 = 0x70;
    pub const KEY_F22: u8 = 0x71;
    pub const KEY_F23: u8 = 0x72;
    pub const KEY_F24: u8 = 0x73;

    pub fn key_name(code: u8) -> &'static str {
        match code {
            0x00 => "None",
            0x04 => "A",
            0x05 => "B",
            0x06 => "C",
            0x07 => "D",
            0x08 => "E",
            0x09 => "F",
            0x0A => "G",
            0x0B => "H",
            0x0C => "I",
            0x0D => "J",
            0x0E => "K",
            0x0F => "L",
            0x10 => "M",
            0x11 => "N",
            0x12 => "O",
            0x13 => "P",
            0x14 => "Q",
            0x15 => "R",
            0x16 => "S",
            0x17 => "T",
            0x18 => "U",
            0x19 => "V",
            0x1A => "W",
            0x1B => "X",
            0x1C => "Y",
            0x1D => "Z",
            0x1E => "1",
            0x1F => "2",
            0x20 => "3",
            0x21 => "4",
            0x22 => "5",
            0x23 => "6",
            0x24 => "7",
            0x25 => "8",
            0x26 => "9",
            0x27 => "0",
            0x28 => "Enter",
            0x29 => "Escape",
            0x2A => "Backspace",
            0x2B => "Tab",
            0x2C => "Space",
            0x2D => "Minus",
            0x2E => "Equal",
            0x2F => "LeftBracket",
            0x30 => "RightBracket",
            0x31 => "Backslash",
            0x33 => "Semicolon",
            0x34 => "Quote",
            0x35 => "Grave",
            0x36 => "Comma",
            0x37 => "Period",
            0x38 => "Slash",
            0x39 => "CapsLock",
            0x3A => "F1",
            0x3B => "F2",
            0x3C => "F3",
            0x3D => "F4",
            0x3E => "F5",
            0x3F => "F6",
            0x40 => "F7",
            0x41 => "F8",
            0x42 => "F9",
            0x43 => "F10",
            0x44 => "F11",
            0x45 => "F12",
            0x46 => "PrintScreen",
            0x47 => "ScrollLock",
            0x48 => "Pause",
            0x49 => "Insert",
            0x4A => "Home",
            0x4B => "PageUp",
            0x4C => "Delete",
            0x4D => "End",
            0x4E => "PageDown",
            0x4F => "Right",
            0x50 => "Left",
            0x51 => "Down",
            0x52 => "Up",
            0x53 => "NumLock",
            0x54 => "KeypadDivide",
            0x55 => "KeypadMultiply",
            0x56 => "KeypadSubtract",
            0x57 => "KeypadAdd",
            0x58 => "KeypadEnter",
            0x59 => "Keypad1",
            0x5A => "Keypad2",
            0x5B => "Keypad3",
            0x5C => "Keypad4",
            0x5D => "Keypad5",
            0x5E => "Keypad6",
            0x5F => "Keypad7",
            0x60 => "Keypad8",
            0x61 => "Keypad9",
            0x62 => "Keypad0",
            0x63 => "KeypadDecimal",
            0x65 => "Application",
            0x68 => "F13",
            0x69 => "F14",
            0x6A => "F15",
            0x6B => "F16",
            0x6C => "F17",
            0x6D => "F18",
            0x6E => "F19",
            0x6F => "F20",
            0x70 => "F21",
            0x71 => "F22",
            0x72 => "F23",
            0x73 => "F24",
            _ => "Unknown",
        }
    }

    pub fn modifier_names(mods: u8) -> Vec<&'static str> {
        let mut names = Vec::new();
        if mods & MOD_LEFT_CTRL != 0 {
            names.push("LCtrl");
        }
        if mods & MOD_LEFT_SHIFT != 0 {
            names.push("LShift");
        }
        if mods & MOD_LEFT_ALT != 0 {
            names.push("LAlt");
        }
        if mods & MOD_LEFT_GUI != 0 {
            names.push("LCmd");
        }
        if mods & MOD_RIGHT_CTRL != 0 {
            names.push("RCtrl");
        }
        if mods & MOD_RIGHT_SHIFT != 0 {
            names.push("RShift");
        }
        if mods & MOD_RIGHT_ALT != 0 {
            names.push("RAlt");
        }
        if mods & MOD_RIGHT_GUI != 0 {
            names.push("RCmd");
        }
        names
    }

    pub fn parse_key_name(name: &str) -> Option<u8> {
        match name.to_lowercase().as_str() {
            "a" => Some(KEY_A),
            "b" => Some(KEY_B),
            "c" => Some(KEY_C),
            "d" => Some(KEY_D),
            "e" => Some(KEY_E),
            "f" => Some(KEY_F),
            "g" => Some(KEY_G),
            "h" => Some(KEY_H),
            "i" => Some(KEY_I),
            "j" => Some(KEY_J),
            "k" => Some(KEY_K),
            "l" => Some(KEY_L),
            "m" => Some(KEY_M),
            "n" => Some(KEY_N),
            "o" => Some(KEY_O),
            "p" => Some(KEY_P),
            "q" => Some(KEY_Q),
            "r" => Some(KEY_R),
            "s" => Some(KEY_S),
            "t" => Some(KEY_T),
            "u" => Some(KEY_U),
            "v" => Some(KEY_V),
            "w" => Some(KEY_W),
            "x" => Some(KEY_X),
            "y" => Some(KEY_Y),
            "z" => Some(KEY_Z),
            "1" => Some(KEY_1),
            "2" => Some(KEY_2),
            "3" => Some(KEY_3),
            "4" => Some(KEY_4),
            "5" => Some(KEY_5),
            "6" => Some(KEY_6),
            "7" => Some(KEY_7),
            "8" => Some(KEY_8),
            "9" => Some(KEY_9),
            "0" => Some(KEY_0),
            "enter" | "return" => Some(KEY_ENTER),
            "esc" | "escape" => Some(KEY_ESC),
            "backspace" => Some(KEY_BACKSPACE),
            "tab" => Some(KEY_TAB),
            "space" => Some(KEY_SPACE),
            "f1" => Some(KEY_F1),
            "f2" => Some(KEY_F2),
            "f3" => Some(KEY_F3),
            "f4" => Some(KEY_F4),
            "f5" => Some(KEY_F5),
            "f6" => Some(KEY_F6),
            "f7" => Some(KEY_F7),
            "f8" => Some(KEY_F8),
            "f9" => Some(KEY_F9),
            "f10" => Some(KEY_F10),
            "f11" => Some(KEY_F11),
            "f12" => Some(KEY_F12),
            "f13" => Some(KEY_F13),
            "f14" => Some(KEY_F14),
            "f15" => Some(KEY_F15),
            "f16" => Some(KEY_F16),
            "f17" => Some(KEY_F17),
            "f18" => Some(KEY_F18),
            "f19" => Some(KEY_F19),
            "f20" => Some(KEY_F20),
            "f21" => Some(KEY_F21),
            "f22" => Some(KEY_F22),
            "f23" => Some(KEY_F23),
            "f24" => Some(KEY_F24),
            "left" => Some(KEY_LEFT),
            "right" => Some(KEY_RIGHT),
            "up" => Some(KEY_UP),
            "down" => Some(KEY_DOWN),
            // Punctuation and special keys
            "minus" | "-" => Some(0x2D),
            "equal" | "=" => Some(0x2E),
            "leftbracket" | "[" => Some(0x2F),
            "rightbracket" | "]" => Some(0x30),
            "backslash" | "\\" => Some(0x31),
            "semicolon" | ";" => Some(0x33),
            "quote" | "'" => Some(0x34),
            "grave" | "`" => Some(0x35),
            "comma" | "," => Some(0x36),
            "period" | "." => Some(0x37),
            "slash" | "/" => Some(0x38),
            "capslock" => Some(0x39),
            "printscreen" | "print-screen" | "prtsc" | "prtscr" => Some(KEY_PRINTSCREEN),
            "scrolllock" | "scroll-lock" => Some(KEY_SCROLLLOCK),
            "pause" => Some(KEY_PAUSE),
            "insert" | "ins" => Some(KEY_INSERT),
            "home" => Some(KEY_HOME),
            "pageup" | "page-up" | "pgup" => Some(KEY_PAGEUP),
            "delete" | "del" | "deleteforward" | "delete-forward" => Some(KEY_DELETE),
            "end" => Some(KEY_END),
            "pagedown" | "page-down" | "pgdn" => Some(KEY_PAGEDOWN),
            "numlock" | "num-lock" => Some(KEY_NUMLOCK),
            "keypad-divide" | "keypad-slash" => Some(KEY_KEYPAD_DIVIDE),
            "keypad-multiply" | "keypad-asterisk" | "keypad-star" => Some(KEY_KEYPAD_MULTIPLY),
            "keypad-subtract" | "keypad-minus" => Some(KEY_KEYPAD_SUBTRACT),
            "keypad-plus" | "keypad-add" => Some(KEY_KEYPAD_ADD),
            "keypad-enter" => Some(KEY_KEYPAD_ENTER),
            "keypad-1" => Some(KEY_KEYPAD_1),
            "keypad-2" => Some(KEY_KEYPAD_2),
            "keypad-3" => Some(KEY_KEYPAD_3),
            "keypad-4" => Some(KEY_KEYPAD_4),
            "keypad-5" => Some(KEY_KEYPAD_5),
            "keypad-6" => Some(KEY_KEYPAD_6),
            "keypad-7" => Some(KEY_KEYPAD_7),
            "keypad-8" => Some(KEY_KEYPAD_8),
            "keypad-9" => Some(KEY_KEYPAD_9),
            "keypad-0" => Some(KEY_KEYPAD_0),
            "keypad-decimal" | "keypad-period" | "keypad-dot" => Some(KEY_KEYPAD_DECIMAL),
            "application" | "menu" => Some(KEY_APPLICATION),
            _ => None,
        }
    }
}

/// Parsed pedal key action (modifier bitmask + HID usage).
#[derive(Debug, Clone)]
pub struct KeyAction {
    pub modifiers: u8,
    pub key: u8,
}

impl KeyAction {
    pub fn from_string(s: &str) -> Result<Self> {
        // Validate input is not empty or whitespace-only
        let s = s.trim();
        if s.is_empty() {
            return Err(anyhow!("Key action cannot be empty"));
        }

        // Validate no leading or trailing '+' (would produce empty parts)
        if s.starts_with('+') || s.ends_with('+') {
            return Err(anyhow!(
                "Key action cannot start or end with '+': \"{}\"",
                s
            ));
        }

        // Validate no consecutive '+' characters (e.g., "cmd++c")
        if s.contains("++") {
            return Err(anyhow!(
                "Key action contains empty modifier (consecutive '+'): \"{}\"",
                s
            ));
        }

        let parts: Vec<&str> = s.split('+').collect();
        let mut modifiers = 0u8;
        let mut key = 0u8;

        for (i, part) in parts.iter().enumerate() {
            let part = part.trim().to_lowercase();
            if part.is_empty() {
                // Extra safety check for whitespace-only parts like "cmd + + c"
                return Err(anyhow!("Key action contains empty component: \"{}\"", s));
            }
            if i == parts.len() - 1 {
                // Last part is the key
                key = usb_hid::parse_key_name(&part)
                    .ok_or_else(|| anyhow!("Unknown key: \"{}\"", part))?;
            } else {
                // Modifier
                match part.as_str() {
                    "cmd" | "command" | "gui" | "meta" | "super" => {
                        modifiers |= usb_hid::MOD_LEFT_GUI;
                    }
                    "ctrl" | "control" => {
                        modifiers |= usb_hid::MOD_LEFT_CTRL;
                    }
                    "shift" => {
                        modifiers |= usb_hid::MOD_LEFT_SHIFT;
                    }
                    "alt" | "option" | "opt" => {
                        modifiers |= usb_hid::MOD_LEFT_ALT;
                    }
                    _ => return Err(anyhow!("Unknown modifier: \"{}\"", part)),
                }
            }
        }

        Ok(Self { modifiers, key })
    }
}

/// Physical pedal select used in Programming-mode writes.
///
/// Captured selectors: A = `0x01`, B = `0x02`, C = `0x03`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pedal {
    /// Byte 0 = `0x01`.
    A,
    /// Byte 0 = `0x02`.
    B,
    /// Byte 0 = `0x03` (captured Pedal C→a).
    C,
}

impl Pedal {
    /// Parse an unambiguous pedal spelling (`a` / `b` / `c`, case-insensitive).
    pub fn from_string(s: &str) -> Result<Self> {
        s.parse()
    }

    /// Request-6 pedal-select byte.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::A => 0x01,
            Self::B => 0x02,
            Self::C => 0x03,
        }
    }
}

impl FromStr for Pedal {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "a" => Ok(Self::A),
            "b" => Ok(Self::B),
            "c" => Ok(Self::C),
            "" => Err(anyhow!("Pedal cannot be empty")),
            other => Err(anyhow!(
                "Unsupported pedal \"{other}\": only A, B, and C are programming targets"
            )),
        }
    }
}

impl fmt::Display for Pedal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::A => f.write_str("A"),
            Self::B => f.write_str("B"),
            Self::C => f.write_str("C"),
        }
    }
}

/// Programming-mode modifier token (`F0`–`F7`).
///
/// Canonical press order is `F0`…`F7`. Release uses the reverse of the
/// modifiers that were pressed. Captured physical release order varied across
/// multi-modifier rows, so encoding preserves semantic validity rather than
/// hardcoding one operator's release order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProgramModifier {
    LeftCtrl = 0,
    LeftShift = 1,
    LeftAlt = 2,
    LeftGui = 3,
    RightCtrl = 4,
    RightShift = 5,
    RightAlt = 6,
    RightGui = 7,
}

impl ProgramModifier {
    /// Programming-mode token byte (`0xF0`–`0xF7`).
    #[must_use]
    pub const fn token(self) -> u8 {
        0xF0 | (self as u8)
    }

    /// Canonical display spelling (`ctrl`, `rshift`, …).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LeftCtrl => "ctrl",
            Self::LeftShift => "shift",
            Self::LeftAlt => "alt",
            Self::LeftGui => "gui",
            Self::RightCtrl => "rctrl",
            Self::RightShift => "rshift",
            Self::RightAlt => "ralt",
            Self::RightGui => "rgui",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "ctrl" | "control" | "lctrl" => Some(Self::LeftCtrl),
            "shift" | "lshift" => Some(Self::LeftShift),
            "alt" | "option" | "lalt" => Some(Self::LeftAlt),
            "gui" | "win" | "cmd" | "lgui" => Some(Self::LeftGui),
            "rctrl" => Some(Self::RightCtrl),
            "rshift" => Some(Self::RightShift),
            "ralt" => Some(Self::RightAlt),
            "rgui" | "rwin" => Some(Self::RightGui),
            _ => None,
        }
    }

    const fn bit(self) -> u8 {
        1 << (self as u8)
    }

    const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::LeftCtrl),
            1 => Some(Self::LeftShift),
            2 => Some(Self::LeftAlt),
            3 => Some(Self::LeftGui),
            4 => Some(Self::RightCtrl),
            5 => Some(Self::RightShift),
            6 => Some(Self::RightAlt),
            7 => Some(Self::RightGui),
            _ => None,
        }
    }
}

/// Set of Programming-mode modifiers, stored in canonical `F0`–`F7` bit order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ProgramModifiers(u8);

impl ProgramModifiers {
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(self, modifier: ProgramModifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    #[must_use]
    pub const fn count(self) -> usize {
        self.0.count_ones() as usize
    }

    fn insert(&mut self, modifier: ProgramModifier) -> Result<()> {
        if self.contains(modifier) {
            return Err(anyhow!(
                "Malformed programming action: duplicate modifier {}",
                modifier.as_str()
            ));
        }
        self.0 |= modifier.bit();
        Ok(())
    }

    /// Canonical press order (`F0` … `F7` of the bits that are set).
    fn press_order(self) -> impl Iterator<Item = ProgramModifier> {
        (0u8..8).filter_map(move |index| {
            let modifier = ProgramModifier::from_index(index)?;
            self.contains(modifier).then_some(modifier)
        })
    }

    /// Canonical release order: reverse of [`Self::press_order`].
    fn release_order(self) -> impl Iterator<Item = ProgramModifier> {
        (0u8..8).rev().filter_map(move |index| {
            let modifier = ProgramModifier::from_index(index)?;
            self.contains(modifier).then_some(modifier)
        })
    }
}

/// One keyed chord: optional modifiers plus exactly one standard keyboard key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgramChord {
    pub modifiers: ProgramModifiers,
    pub key: u8,
}

impl ProgramChord {
    fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(anyhow!(
                "Malformed programming action: empty chord (comma-separated parts cannot be empty)"
            ));
        }
        if s.starts_with('+') || s.ends_with('+') {
            return Err(anyhow!(
                "Malformed programming action: chord cannot start or end with '+': \"{s}\""
            ));
        }
        if s.contains("++") {
            return Err(anyhow!(
                "Malformed programming action: empty modifier (consecutive '+'): \"{s}\""
            ));
        }

        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        if parts.iter().any(|part| part.is_empty()) {
            return Err(anyhow!(
                "Malformed programming action: empty component: \"{s}\""
            ));
        }

        let Some((key_part, mod_parts)) = parts.split_last() else {
            return Err(anyhow!("Programming action cannot be empty"));
        };
        let key_part = key_part.to_ascii_lowercase();

        if mod_parts.is_empty() && ProgramModifier::from_name(&key_part).is_some() {
            return Err(anyhow!(
                "Modifier-only programming action is not allowed: \"{s}\""
            ));
        }

        let mut modifiers = ProgramModifiers::empty();
        for part in mod_parts {
            let name = part.to_ascii_lowercase();
            let Some(modifier) = ProgramModifier::from_name(&name) else {
                return Err(anyhow!(
                    "Unknown modifier: \"{part}\" (use ctrl/shift/alt/gui or rctrl/rshift/ralt/rgui)"
                ));
            };
            modifiers.insert(modifier)?;
        }

        if ProgramModifier::from_name(&key_part).is_some() {
            return Err(anyhow!(
                "Modifier-only programming action is not allowed: \"{s}\""
            ));
        }

        let key = parse_program_key(&key_part)?;
        Ok(Self { modifiers, key })
    }

    fn encode_body(self, body: &mut Vec<u8>) {
        for modifier in self.modifiers.press_order() {
            body.push(modifier.token());
        }
        body.push(self.key);
        body.push(PROGRAM_KEY_UP);
        body.push(self.key);
        for modifier in self.modifiers.release_order() {
            body.push(PROGRAM_KEY_UP);
            body.push(modifier.token());
        }
    }
}

impl fmt::Display for ProgramChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for modifier in self.modifiers.press_order() {
            write!(f, "{}+", modifier.as_str())?;
        }
        f.write_str(program_key_name(self.key))
    }
}

/// Captured 9-byte mouse envelope (byte 1 = `0x20`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseMapping {
    LeftClick,
    RightClick,
    MiddleClick,
    ScrollUp,
    ScrollDown,
}

impl MouseMapping {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "left-click" | "leftclick" => Some(Self::LeftClick),
            "right-click" | "rightclick" => Some(Self::RightClick),
            "middle-click" | "middleclick" => Some(Self::MiddleClick),
            "scroll-up" | "scrollup" => Some(Self::ScrollUp),
            "scroll-down" | "scrolldown" => Some(Self::ScrollDown),
            _ => None,
        }
    }

    /// Button bitmap at offset 5 (`01` / `02` / `04` / `00`).
    #[must_use]
    pub const fn button_byte(self) -> u8 {
        match self {
            Self::LeftClick => 0x01,
            Self::RightClick => 0x02,
            Self::MiddleClick => 0x04,
            Self::ScrollUp | Self::ScrollDown => 0x00,
        }
    }

    /// Scroll byte at offset 8 (`00` / `01` / `ff`).
    #[must_use]
    pub const fn scroll_byte(self) -> u8 {
        match self {
            Self::LeftClick | Self::RightClick | Self::MiddleClick => 0x00,
            Self::ScrollUp => 0x01,
            Self::ScrollDown => 0xFF,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LeftClick => "left-click",
            Self::RightClick => "right-click",
            Self::MiddleClick => "middle-click",
            Self::ScrollUp => "scroll-up",
            Self::ScrollDown => "scroll-down",
        }
    }
}

/// Programming-mode action: `clear`, mouse, one modifier, or keyed chords.
///
/// This is not a Play-mode [`KeyAction`]. Previously valid spellings (`a`,
/// `b`, `ctrl+a`, `a,b`, `ctrl+a,b`) still parse and still emit the captured
/// payloads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProgramAction {
    /// Captured five-byte clear: `[pedal, 00, 00, 00, 00]`. Cannot be combined.
    Clear,
    /// Captured 9-byte mouse click or self-scroll. Cannot be combined.
    Mouse(MouseMapping),
    /// One modifier with no key (`ctrl`, `shift`, …). Captured as `Fn FE Fn`.
    ModifierOnly(ProgramModifier),
    /// One or more comma-separated chords.
    Macro(Vec<ProgramChord>),
}

impl ProgramAction {
    /// Parse a programming action spelling.
    ///
    /// Grammar: `clear`, a mouse name, a single modifier, or
    /// `chord[,chord…]` where each chord is `[modifier+…]key`.
    /// `clear` and mouse names cannot be combined with other chords.
    pub fn from_string(s: &str) -> Result<Self> {
        s.parse()
    }

    /// Number of key taps in a macro (`0` for `clear` and mouse).
    #[must_use]
    pub fn key_tap_count(&self) -> usize {
        match self {
            Self::Clear | Self::Mouse(_) => 0,
            Self::ModifierOnly(_) => 1,
            Self::Macro(chords) => chords.len(),
        }
    }
}

impl FromStr for ProgramAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("Programming action cannot be empty"));
        }
        if trimmed.eq_ignore_ascii_case("clear") {
            return Ok(Self::Clear);
        }

        let lower = trimmed.to_ascii_lowercase();
        if let Some(mouse) = MouseMapping::from_name(&lower) {
            return Ok(Self::Mouse(mouse));
        }

        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts
            .iter()
            .any(|part| part.trim().eq_ignore_ascii_case("clear"))
        {
            return Err(anyhow!(
                "clear cannot be combined with other actions (duplicate or conflicting clear)"
            ));
        }

        let mouse_parts: Vec<&str> = parts
            .iter()
            .map(|part| part.trim())
            .filter(|part| MouseMapping::from_name(&part.to_ascii_lowercase()).is_some())
            .collect();
        if !mouse_parts.is_empty() {
            return Err(anyhow!(
                "Mouse actions cannot be combined with modifiers or other keys (use left-click, right-click, middle-click, scroll-up, or scroll-down alone)"
            ));
        }

        if parts.len() == 1 {
            let name = parts[0].trim().to_ascii_lowercase();
            if let Some(modifier) = ProgramModifier::from_name(&name) {
                return Ok(Self::ModifierOnly(modifier));
            }
        }

        let mut chords = Vec::with_capacity(parts.len());
        for part in parts {
            chords.push(ProgramChord::parse(part)?);
        }
        if chords.is_empty() {
            return Err(anyhow!("Programming action cannot be empty"));
        }
        Ok(Self::Macro(chords))
    }
}

impl fmt::Display for ProgramAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clear => f.write_str("clear"),
            Self::Mouse(mouse) => f.write_str(mouse.as_str()),
            Self::ModifierOnly(modifier) => f.write_str(modifier.as_str()),
            Self::Macro(chords) => {
                for (index, chord) in chords.iter().enumerate() {
                    if index > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{chord}")?;
                }
                Ok(())
            }
        }
    }
}

/// Programming-mode key-up / wrap token.
pub const PROGRAM_KEY_UP: u8 = 0xFE;

/// Statically recovered journal copy limit (`SendMacro` buffer is `0x800` bytes).
pub const PROGRAM_JOURNAL_LIMIT: usize = 0x800;

fn f13_f24_device_limitation(name: &str) -> anyhow::Error {
    anyhow!(
        "Device limitation: \"{name}\" is F13-F24, which this Savant Elite accepted as a write but produced no Play event. F1-F12 remain supported; F13-F24 are not programmable on this unit."
    )
}

fn is_f13_f24_program_name(name: &str) -> bool {
    matches!(
        name,
        "f13"
            | "f14"
            | "f15"
            | "f16"
            | "f17"
            | "f18"
            | "f19"
            | "f20"
            | "f21"
            | "f22"
            | "f23"
            | "f24"
    )
}

fn is_f13_f24_usage(code: u8) -> bool {
    matches!(code, 0x68..=0x73)
}

fn parse_program_key(name: &str) -> Result<u8> {
    if MouseMapping::from_name(name).is_some() {
        return Err(anyhow!(
            "Mouse actions cannot be combined with modifiers or other keys (use left-click, right-click, middle-click, scroll-up, or scroll-down alone)"
        ));
    }
    if is_unsupported_program_token(name) {
        return Err(anyhow!(
            "Unsupported programming key \"{name}\": consumer/media, mouse, power/sleep, delays, and repeats are out of scope"
        ));
    }
    if is_f13_f24_program_name(name) {
        return Err(f13_f24_device_limitation(name));
    }
    if name.starts_with("0x") || name.starts_with("0X") {
        return Err(anyhow!(
            "Arbitrary numeric usage codes are not allowed: \"{name}\""
        ));
    }
    if name.len() > 1 && name.bytes().all(|b| b.is_ascii_digit()) {
        return Err(anyhow!(
            "Arbitrary numeric usage codes are not allowed: \"{name}\""
        ));
    }
    usb_hid::parse_key_name(name).ok_or_else(|| anyhow!("Unknown key: \"{name}\""))
}

fn is_unsupported_program_token(name: &str) -> bool {
    matches!(
        name,
        "play"
            | "playpause"
            | "media"
            | "mediaplay"
            | "medianext"
            | "mediaprev"
            | "nexttrack"
            | "prevtrack"
            | "volume"
            | "volumeup"
            | "volumedown"
            | "mute"
            | "consumer"
            | "mouse"
            | "click"
            | "wheel"
            | "power"
            | "sleep"
            | "wakeup"
            | "delay"
            | "wait"
            | "repeat"
            | "hold"
    )
}

fn program_key_name(code: u8) -> &'static str {
    match code {
        0x04 => "a",
        0x05 => "b",
        0x06 => "c",
        0x07 => "d",
        0x08 => "e",
        0x09 => "f",
        0x0A => "g",
        0x0B => "h",
        0x0C => "i",
        0x0D => "j",
        0x0E => "k",
        0x0F => "l",
        0x10 => "m",
        0x11 => "n",
        0x12 => "o",
        0x13 => "p",
        0x14 => "q",
        0x15 => "r",
        0x16 => "s",
        0x17 => "t",
        0x18 => "u",
        0x19 => "v",
        0x1A => "w",
        0x1B => "x",
        0x1C => "y",
        0x1D => "z",
        0x1E => "1",
        0x1F => "2",
        0x20 => "3",
        0x21 => "4",
        0x22 => "5",
        0x23 => "6",
        0x24 => "7",
        0x25 => "8",
        0x26 => "9",
        0x27 => "0",
        0x28 => "enter",
        0x29 => "esc",
        0x2A => "backspace",
        0x2B => "tab",
        0x2C => "space",
        0x2D => "minus",
        0x2E => "equal",
        0x2F => "leftbracket",
        0x30 => "rightbracket",
        0x31 => "backslash",
        0x33 => "semicolon",
        0x34 => "quote",
        0x35 => "grave",
        0x36 => "comma",
        0x37 => "period",
        0x38 => "slash",
        0x39 => "capslock",
        0x3A => "f1",
        0x3B => "f2",
        0x3C => "f3",
        0x3D => "f4",
        0x3E => "f5",
        0x3F => "f6",
        0x40 => "f7",
        0x41 => "f8",
        0x42 => "f9",
        0x43 => "f10",
        0x44 => "f11",
        0x45 => "f12",
        0x46 => "printscreen",
        0x47 => "scrolllock",
        0x48 => "pause",
        0x49 => "insert",
        0x4A => "home",
        0x4B => "pageup",
        0x4C => "delete",
        0x4D => "end",
        0x4E => "pagedown",
        0x4F => "right",
        0x50 => "left",
        0x51 => "down",
        0x52 => "up",
        0x53 => "numlock",
        0x54 => "keypad-divide",
        0x55 => "keypad-multiply",
        0x56 => "keypad-subtract",
        0x57 => "keypad-plus",
        0x58 => "keypad-enter",
        0x59 => "keypad-1",
        0x5A => "keypad-2",
        0x5B => "keypad-3",
        0x5C => "keypad-4",
        0x5D => "keypad-5",
        0x5E => "keypad-6",
        0x5F => "keypad-7",
        0x60 => "keypad-8",
        0x61 => "keypad-9",
        0x62 => "keypad-0",
        0x63 => "keypad-decimal",
        0x65 => "application",
        0x68 => "f13",
        0x69 => "f14",
        0x6A => "f15",
        0x6B => "f16",
        0x6C => "f17",
        0x6D => "f18",
        0x6E => "f19",
        0x6F => "f20",
        0x70 => "f21",
        0x71 => "f22",
        0x72 => "f23",
        0x73 => "f24",
        _ => "unknown",
    }
}

/// Encode a Programming-mode request-6 payload.
///
/// # Byte rules
///
/// * Byte 0 is the pedal selector (`01`/`02`/`03`). Keyboard byte 1 is `00`.
/// * `clear` is exactly `[pedal, 00, 00, 00, 00]` and has no body.
/// * Mouse mappings are exactly
///   `[pedal, 20, 00, 04, 00, button, 00, 00, scroll]`.
/// * A single modifier with no key (`ctrl`) is the `N == 1` header plus
///   `Fn FE Fn`. Multi-modifier-only chords are rejected (no capture).
/// * Each chord body is modifier-down tokens (`F0`–`F7` in canonical order),
///   then `KEY FE KEY`, then `FE MOD` for each modifier in canonical reverse
///   order.
/// * A comma-separated action concatenates chord bodies. `N` is the number of
///   key taps (chords). `M` is the modifier count of the single chord when
///   `N == 1`.
/// * If `N == 1`, bytes 2–4 are `00`, `M+1`, `2*(M+1)` (captured `01/02`,
///   `02/04`, `03/06`, `04/08`, `05/0A`).
/// * If `N >= 2`, bytes 2–4 are `body_len` as a 16-bit big-endian value and
///   `00` (captured `06/00`, `09/00`, `0C/00`, `0F/00`, …).
/// * Body length must not exceed [`PROGRAM_JOURNAL_LIMIT`] (`0x800`).
///
/// Previously valid basic spellings still emit the captured payloads. Empty,
/// multi-modifier-only, malformed, conflicting `clear`, over-length, unknown
/// keys, and F13-F24 (hardware: write accepted, no Play event) fail before USB.
pub fn encode_program(pedal: Pedal, action: &ProgramAction) -> Result<Vec<u8>> {
    match action {
        ProgramAction::Clear => Ok(vec![pedal.as_byte(), 0x00, 0x00, 0x00, 0x00]),
        ProgramAction::Mouse(mouse) => Ok(vec![
            pedal.as_byte(),
            0x20,
            0x00,
            0x04,
            0x00,
            mouse.button_byte(),
            0x00,
            0x00,
            mouse.scroll_byte(),
        ]),
        ProgramAction::ModifierOnly(modifier) => {
            let token = modifier.token();
            Ok(vec![
                pedal.as_byte(),
                0x00,
                0x00,
                0x01,
                0x02,
                token,
                PROGRAM_KEY_UP,
                token,
            ])
        }
        ProgramAction::Macro(chords) => {
            if chords.is_empty() {
                return Err(anyhow!("Programming action cannot be empty"));
            }
            for chord in chords {
                if is_f13_f24_usage(chord.key) {
                    return Err(f13_f24_device_limitation(program_key_name(chord.key)));
                }
            }
            let mut body = Vec::new();
            for chord in chords {
                chord.encode_body(&mut body);
            }
            if body.len() > PROGRAM_JOURNAL_LIMIT {
                return Err(anyhow!(
                    "Programming action exceeds the 0x800 journal buffer ({} bytes)",
                    body.len()
                ));
            }
            let n = chords.len();
            let mut payload = Vec::with_capacity(5 + body.len());
            payload.push(pedal.as_byte());
            payload.push(0x00);
            if n == 1 {
                let modifier_count = chords[0].modifiers.count();
                let groups = u8::try_from(modifier_count + 1)
                    .map_err(|_| anyhow!("Programming action exceeds the 0x800 journal buffer"))?;
                payload.push(0x00);
                payload.push(groups);
                payload.push(groups.saturating_mul(2));
            } else {
                payload.push(
                    u8::try_from(body.len() >> 8).map_err(|_| {
                        anyhow!("Programming action exceeds the 0x800 journal buffer")
                    })?,
                );
                payload.push(
                    u8::try_from(body.len() & 0xFF).map_err(|_| {
                        anyhow!("Programming action exceeds the 0x800 journal buffer")
                    })?,
                );
                payload.push(0x00);
            }
            payload.extend_from_slice(&body);
            Ok(payload)
        }
    }
}

/// Parse pedal and action spellings, then encode.
///
/// Fails on unsupported spellings before any USB layer is involved.
pub fn encode_program_from_str(pedal: &str, action: &str) -> Result<Vec<u8>> {
    let action = ProgramAction::from_string(action)?;
    encode_program(Pedal::from_string(pedal)?, &action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_action_cmd_c() {
        let action = KeyAction::from_string("cmd+c").unwrap();
        assert_eq!(action.modifiers, usb_hid::MOD_LEFT_GUI);
        assert_eq!(action.key, usb_hid::KEY_C);
    }

    #[test]
    fn parse_key_action_multi_modifiers() {
        let action = KeyAction::from_string("ctrl+shift+alt+f12").unwrap();
        assert_eq!(
            action.modifiers,
            usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_SHIFT | usb_hid::MOD_LEFT_ALT
        );
        assert_eq!(action.key, usb_hid::KEY_F12);
    }

    #[test]
    fn parse_key_action_aliases() {
        let a1 = KeyAction::from_string("option+a").unwrap();
        let a2 = KeyAction::from_string("opt+a").unwrap();
        let a3 = KeyAction::from_string("alt+a").unwrap();

        assert_eq!(a1.modifiers, usb_hid::MOD_LEFT_ALT);
        assert_eq!(a2.modifiers, usb_hid::MOD_LEFT_ALT);
        assert_eq!(a3.modifiers, usb_hid::MOD_LEFT_ALT);

        assert_eq!(a1.key, usb_hid::KEY_A);
        assert_eq!(a2.key, usb_hid::KEY_A);
        assert_eq!(a3.key, usb_hid::KEY_A);
    }

    #[test]
    fn parse_key_action_rejects_unknown_modifier() {
        let err = KeyAction::from_string("hyper+a").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("unknown modifier"));
    }

    #[test]
    fn parse_key_action_rejects_unknown_key() {
        let err = KeyAction::from_string("cmd+notakey").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("unknown key"));
    }

    #[test]
    fn parse_key_action_rejects_empty() {
        let err = KeyAction::from_string("").unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn parse_key_action_rejects_whitespace_only() {
        let err = KeyAction::from_string("   ").unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn parse_key_action_rejects_leading_plus() {
        let err = KeyAction::from_string("+c").unwrap_err();
        assert!(err.to_string().contains("cannot start or end with"));
    }

    #[test]
    fn parse_key_action_rejects_trailing_plus() {
        let err = KeyAction::from_string("cmd+").unwrap_err();
        assert!(err.to_string().contains("cannot start or end with"));
    }

    #[test]
    fn parse_key_action_rejects_just_plus() {
        let err = KeyAction::from_string("+").unwrap_err();
        assert!(err.to_string().contains("cannot start or end with"));
    }

    #[test]
    fn parse_key_action_rejects_double_plus() {
        let err = KeyAction::from_string("cmd++c").unwrap_err();
        assert!(err.to_string().contains("consecutive"));
    }

    #[test]
    fn parse_key_name_punctuation() {
        assert_eq!(usb_hid::parse_key_name("-"), Some(0x2D));
        assert_eq!(usb_hid::parse_key_name("="), Some(0x2E));
        assert_eq!(usb_hid::parse_key_name("escape"), Some(usb_hid::KEY_ESC));
    }

    #[test]
    fn parse_key_name_all_letters() {
        // USB HID key codes for a-z are 0x04-0x1D
        let expected_codes: Vec<(char, u8)> = ('a'..='z').zip(0x04u8..=0x1D).collect();

        for (letter, expected) in expected_codes {
            let result = usb_hid::parse_key_name(&letter.to_string());
            assert_eq!(
                result,
                Some(expected),
                "Failed for letter '{}': expected 0x{:02X}, got {:?}",
                letter,
                expected,
                result
            );
        }
    }

    #[test]
    fn parse_key_name_all_numbers() {
        // USB HID: 1-9 are 0x1E-0x26, 0 is 0x27
        for (num, expected) in ('1'..='9').zip(0x1Eu8..=0x26) {
            let result = usb_hid::parse_key_name(&num.to_string());
            assert_eq!(
                result,
                Some(expected),
                "Failed for number '{}': expected 0x{:02X}, got {:?}",
                num,
                expected,
                result
            );
        }
        // Zero is special
        assert_eq!(usb_hid::parse_key_name("0"), Some(0x27));
    }

    #[test]
    fn parse_key_name_all_function_keys() {
        // USB HID: F1-F12 are 0x3A-0x45; F13-F24 are 0x68-0x73
        for (i, expected) in (1u8..=12).zip(0x3Au8..=0x45) {
            let key_name = format!("f{}", i);
            let result = usb_hid::parse_key_name(&key_name);
            assert_eq!(
                result,
                Some(expected),
                "Failed for '{}': expected 0x{:02X}, got {:?}",
                key_name,
                expected,
                result
            );
        }
        for (i, expected) in (13u8..=24).zip(0x68u8..=0x73) {
            let key_name = format!("f{}", i);
            let result = usb_hid::parse_key_name(&key_name);
            assert_eq!(
                result,
                Some(expected),
                "Failed for '{}': expected 0x{:02X}, got {:?}",
                key_name,
                expected,
                result
            );
        }
    }

    #[test]
    fn parse_key_name_case_insensitive() {
        // All key names should be case-insensitive
        assert_eq!(usb_hid::parse_key_name("A"), usb_hid::parse_key_name("a"));
        assert_eq!(
            usb_hid::parse_key_name("ENTER"),
            usb_hid::parse_key_name("enter")
        );
        assert_eq!(
            usb_hid::parse_key_name("F12"),
            usb_hid::parse_key_name("f12")
        );
        assert_eq!(
            usb_hid::parse_key_name("SPACE"),
            usb_hid::parse_key_name("space")
        );
        assert_eq!(
            usb_hid::parse_key_name("Tab"),
            usb_hid::parse_key_name("TAB")
        );
    }

    #[test]
    fn parse_key_name_special_keys() {
        // Verify special key mappings
        assert_eq!(usb_hid::parse_key_name("enter"), Some(usb_hid::KEY_ENTER));
        assert_eq!(usb_hid::parse_key_name("return"), Some(usb_hid::KEY_ENTER));
        assert_eq!(usb_hid::parse_key_name("esc"), Some(usb_hid::KEY_ESC));
        assert_eq!(usb_hid::parse_key_name("escape"), Some(usb_hid::KEY_ESC));
        assert_eq!(
            usb_hid::parse_key_name("backspace"),
            Some(usb_hid::KEY_BACKSPACE)
        );
        assert_eq!(usb_hid::parse_key_name("tab"), Some(usb_hid::KEY_TAB));
        assert_eq!(usb_hid::parse_key_name("space"), Some(usb_hid::KEY_SPACE));
    }

    #[test]
    fn parse_key_name_arrow_keys() {
        assert_eq!(usb_hid::parse_key_name("left"), Some(usb_hid::KEY_LEFT));
        assert_eq!(usb_hid::parse_key_name("right"), Some(usb_hid::KEY_RIGHT));
        assert_eq!(usb_hid::parse_key_name("up"), Some(usb_hid::KEY_UP));
        assert_eq!(usb_hid::parse_key_name("down"), Some(usb_hid::KEY_DOWN));
    }

    #[test]
    fn parse_key_name_returns_none_for_unknown() {
        assert_eq!(usb_hid::parse_key_name("notakey"), None);
        assert_eq!(usb_hid::parse_key_name(""), None);
        assert_eq!(usb_hid::parse_key_name("f25"), None);
        assert_eq!(usb_hid::parse_key_name("ctrl"), None); // Modifier, not key
        assert_eq!(usb_hid::parse_key_name("cmd"), None); // Modifier, not key
        assert_eq!(usb_hid::parse_key_name("play"), None); // Consumer/media
    }

    #[test]
    fn key_action_cmd_modifier_aliases() {
        // All cmd aliases should map to MOD_LEFT_GUI
        for alias in ["cmd", "command", "gui", "meta", "super"] {
            let action = KeyAction::from_string(&format!("{}+a", alias)).unwrap();
            assert_eq!(
                action.modifiers,
                usb_hid::MOD_LEFT_GUI,
                "Failed for '{}'",
                alias
            );
            assert_eq!(action.key, usb_hid::KEY_A);
        }
    }

    #[test]
    fn key_action_ctrl_modifier_aliases() {
        // All ctrl aliases should map to MOD_LEFT_CTRL
        for alias in ["ctrl", "control"] {
            let action = KeyAction::from_string(&format!("{}+a", alias)).unwrap();
            assert_eq!(
                action.modifiers,
                usb_hid::MOD_LEFT_CTRL,
                "Failed for '{}'",
                alias
            );
        }
    }

    #[test]
    fn key_action_alt_modifier_aliases() {
        // All alt aliases should map to MOD_LEFT_ALT
        for alias in ["alt", "option", "opt"] {
            let action = KeyAction::from_string(&format!("{}+a", alias)).unwrap();
            assert_eq!(
                action.modifiers,
                usb_hid::MOD_LEFT_ALT,
                "Failed for '{}'",
                alias
            );
        }
    }

    #[test]
    fn key_action_shift_modifier() {
        let action = KeyAction::from_string("shift+a").unwrap();
        assert_eq!(action.modifiers, usb_hid::MOD_LEFT_SHIFT);
        assert_eq!(action.key, usb_hid::KEY_A);
    }

    #[test]
    fn key_action_all_four_modifiers() {
        // Combine all four modifiers
        let action = KeyAction::from_string("cmd+ctrl+shift+alt+a").unwrap();
        let expected = usb_hid::MOD_LEFT_GUI
            | usb_hid::MOD_LEFT_CTRL
            | usb_hid::MOD_LEFT_SHIFT
            | usb_hid::MOD_LEFT_ALT;
        assert_eq!(action.modifiers, expected);
        assert_eq!(action.key, usb_hid::KEY_A);
    }

    #[test]
    fn key_action_modifier_order_independent() {
        // Order of modifiers shouldn't matter
        let action1 = KeyAction::from_string("cmd+ctrl+a").unwrap();
        let action2 = KeyAction::from_string("ctrl+cmd+a").unwrap();
        assert_eq!(action1.modifiers, action2.modifiers);
        assert_eq!(action1.key, action2.key);

        let action3 = KeyAction::from_string("shift+alt+ctrl+cmd+z").unwrap();
        let action4 = KeyAction::from_string("cmd+ctrl+alt+shift+z").unwrap();
        assert_eq!(action3.modifiers, action4.modifiers);
    }

    #[test]
    fn key_action_duplicate_modifiers_idempotent() {
        // Specifying the same modifier twice should be idempotent
        let action1 = KeyAction::from_string("cmd+a").unwrap();
        let action2 = KeyAction::from_string("cmd+cmd+a").unwrap();
        assert_eq!(action1.modifiers, action2.modifiers);
    }

    #[test]
    fn key_action_modifier_case_insensitive() {
        // Modifiers should be case-insensitive
        let action1 = KeyAction::from_string("CMD+a").unwrap();
        let action2 = KeyAction::from_string("cmd+a").unwrap();
        assert_eq!(action1.modifiers, action2.modifiers);

        let action3 = KeyAction::from_string("CTRL+SHIFT+a").unwrap();
        let action4 = KeyAction::from_string("ctrl+shift+a").unwrap();
        assert_eq!(action3.modifiers, action4.modifiers);
    }

    #[test]
    fn key_action_mixed_alias_combinations() {
        // Test mixing different aliases for the same modifier type in combinations
        let action1 = KeyAction::from_string("command+control+a").unwrap();
        assert_eq!(
            action1.modifiers,
            usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_CTRL
        );

        let action2 = KeyAction::from_string("gui+option+a").unwrap();
        assert_eq!(
            action2.modifiers,
            usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_ALT
        );

        let action3 = KeyAction::from_string("meta+opt+shift+a").unwrap();
        assert_eq!(
            action3.modifiers,
            usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_ALT | usb_hid::MOD_LEFT_SHIFT
        );

        let action4 = KeyAction::from_string("super+control+option+a").unwrap();
        assert_eq!(
            action4.modifiers,
            usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_ALT
        );
    }

    #[test]
    fn key_action_two_modifier_combinations() {
        // Exhaustive two-modifier combinations
        let combos = [
            ("cmd+ctrl", usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_CTRL),
            ("cmd+shift", usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_SHIFT),
            ("cmd+alt", usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_ALT),
            (
                "ctrl+shift",
                usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_SHIFT,
            ),
            ("ctrl+alt", usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_ALT),
            ("shift+alt", usb_hid::MOD_LEFT_SHIFT | usb_hid::MOD_LEFT_ALT),
        ];

        for (mods, expected) in combos {
            let input = format!("{}+a", mods);
            let action = KeyAction::from_string(&input).unwrap();
            assert_eq!(
                action.modifiers, expected,
                "Two-mod combo '{}' failed: expected 0x{:02X}, got 0x{:02X}",
                input, expected, action.modifiers
            );
        }
    }

    #[test]
    fn key_action_three_modifier_combinations() {
        // All three-modifier combinations
        let combos = [
            (
                "cmd+ctrl+shift",
                usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_SHIFT,
            ),
            (
                "cmd+ctrl+alt",
                usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_ALT,
            ),
            (
                "cmd+shift+alt",
                usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_SHIFT | usb_hid::MOD_LEFT_ALT,
            ),
            (
                "ctrl+shift+alt",
                usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_SHIFT | usb_hid::MOD_LEFT_ALT,
            ),
        ];

        for (mods, expected) in combos {
            let input = format!("{}+a", mods);
            let action = KeyAction::from_string(&input).unwrap();
            assert_eq!(
                action.modifiers, expected,
                "Three-mod combo '{}' failed: expected 0x{:02X}, got 0x{:02X}",
                input, expected, action.modifiers
            );
        }
    }

    #[test]
    fn key_action_modifiers_with_function_keys() {
        // Test modifiers combined with function keys
        let action1 = KeyAction::from_string("cmd+f1").unwrap();
        assert_eq!(action1.modifiers, usb_hid::MOD_LEFT_GUI);
        assert_eq!(action1.key, usb_hid::KEY_F1);

        let action2 = KeyAction::from_string("ctrl+shift+f5").unwrap();
        assert_eq!(
            action2.modifiers,
            usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_SHIFT
        );
        assert_eq!(action2.key, usb_hid::KEY_F5);

        let action3 = KeyAction::from_string("cmd+alt+f12").unwrap();
        assert_eq!(
            action3.modifiers,
            usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_ALT
        );
        assert_eq!(action3.key, usb_hid::KEY_F12);
    }

    #[test]
    fn key_action_modifiers_with_special_keys() {
        // Test modifiers combined with special keys
        let test_cases = [
            ("cmd+enter", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_ENTER),
            ("ctrl+space", usb_hid::MOD_LEFT_CTRL, usb_hid::KEY_SPACE),
            ("alt+tab", usb_hid::MOD_LEFT_ALT, usb_hid::KEY_TAB),
            (
                "shift+backspace",
                usb_hid::MOD_LEFT_SHIFT,
                usb_hid::KEY_BACKSPACE,
            ),
            ("cmd+escape", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_ESC),
            ("cmd+return", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_ENTER), // alias
            ("cmd+esc", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_ESC),      // alias
        ];

        for (input, expected_mod, expected_key) in test_cases {
            let action = KeyAction::from_string(input).unwrap();
            assert_eq!(
                action.modifiers, expected_mod,
                "Modifier for '{}' failed",
                input
            );
            assert_eq!(action.key, expected_key, "Key for '{}' failed", input);
        }
    }

    #[test]
    fn key_action_modifiers_with_arrow_keys() {
        // Test modifiers combined with arrow keys
        let test_cases = [
            ("cmd+left", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_LEFT),
            ("cmd+right", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_RIGHT),
            ("cmd+up", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_UP),
            ("cmd+down", usb_hid::MOD_LEFT_GUI, usb_hid::KEY_DOWN),
            (
                "cmd+shift+left",
                usb_hid::MOD_LEFT_GUI | usb_hid::MOD_LEFT_SHIFT,
                usb_hid::KEY_LEFT,
            ),
            (
                "ctrl+alt+up",
                usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_ALT,
                usb_hid::KEY_UP,
            ),
        ];

        for (input, expected_mod, expected_key) in test_cases {
            let action = KeyAction::from_string(input).unwrap();
            assert_eq!(
                action.modifiers, expected_mod,
                "Arrow key modifier for '{}' failed",
                input
            );
            assert_eq!(action.key, expected_key, "Arrow key for '{}' failed", input);
        }
    }

    #[test]
    fn key_action_modifiers_with_punctuation() {
        // Test modifiers combined with punctuation keys
        let action1 = KeyAction::from_string("cmd+-").unwrap();
        assert_eq!(action1.modifiers, usb_hid::MOD_LEFT_GUI);
        assert_eq!(action1.key, 0x2D); // minus

        let action2 = KeyAction::from_string("cmd+=").unwrap();
        assert_eq!(action2.modifiers, usb_hid::MOD_LEFT_GUI);
        assert_eq!(action2.key, 0x2E); // equals

        let action3 = KeyAction::from_string("ctrl+shift+-").unwrap();
        assert_eq!(
            action3.modifiers,
            usb_hid::MOD_LEFT_CTRL | usb_hid::MOD_LEFT_SHIFT
        );
    }

    #[test]
    fn key_action_case_variations_all_aliases() {
        // Comprehensive case variations for all aliases
        let test_cases = [
            // GUI variants
            ("CMD+x", usb_hid::MOD_LEFT_GUI),
            ("Cmd+x", usb_hid::MOD_LEFT_GUI),
            ("COMMAND+x", usb_hid::MOD_LEFT_GUI),
            ("Command+x", usb_hid::MOD_LEFT_GUI),
            ("GUI+x", usb_hid::MOD_LEFT_GUI),
            ("Gui+x", usb_hid::MOD_LEFT_GUI),
            ("META+x", usb_hid::MOD_LEFT_GUI),
            ("Meta+x", usb_hid::MOD_LEFT_GUI),
            ("SUPER+x", usb_hid::MOD_LEFT_GUI),
            ("Super+x", usb_hid::MOD_LEFT_GUI),
            // CTRL variants
            ("CTRL+x", usb_hid::MOD_LEFT_CTRL),
            ("Ctrl+x", usb_hid::MOD_LEFT_CTRL),
            ("CONTROL+x", usb_hid::MOD_LEFT_CTRL),
            ("Control+x", usb_hid::MOD_LEFT_CTRL),
            // ALT variants
            ("ALT+x", usb_hid::MOD_LEFT_ALT),
            ("Alt+x", usb_hid::MOD_LEFT_ALT),
            ("OPTION+x", usb_hid::MOD_LEFT_ALT),
            ("Option+x", usb_hid::MOD_LEFT_ALT),
            ("OPT+x", usb_hid::MOD_LEFT_ALT),
            ("Opt+x", usb_hid::MOD_LEFT_ALT),
            // SHIFT variants
            ("SHIFT+x", usb_hid::MOD_LEFT_SHIFT),
            ("Shift+x", usb_hid::MOD_LEFT_SHIFT),
        ];

        for (input, expected_mod) in test_cases {
            let action = KeyAction::from_string(input).unwrap();
            assert_eq!(
                action.modifiers, expected_mod,
                "Case variation '{}' failed: expected 0x{:02X}, got 0x{:02X}",
                input, expected_mod, action.modifiers
            );
        }
    }

    fn encode_ok(pedal: &str, action: &str) -> Vec<u8> {
        encode_program_from_str(pedal, action).expect("expected a valid programming mapping")
    }

    #[test]
    fn encode_original_six_exact_payloads() {
        assert_eq!(
            encode_ok("a", "a"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x04, 0xFE, 0x04]
        );
        assert_eq!(
            encode_ok("A", "b"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x05, 0xFE, 0x05]
        );
        assert_eq!(
            encode_ok("b", "a"),
            [0x02, 0x00, 0x00, 0x01, 0x02, 0x04, 0xFE, 0x04]
        );
        assert_eq!(
            encode_ok("a", "Ctrl+A"),
            [0x01, 0x00, 0x00, 0x02, 0x04, 0xF0, 0x04, 0xFE, 0x04, 0xFE, 0xF0]
        );
        assert_eq!(
            encode_ok("a", "a,b"),
            [0x01, 0x00, 0x00, 0x06, 0x00, 0x04, 0xFE, 0x04, 0x05, 0xFE, 0x05]
        );
        assert_eq!(
            encode_ok("a", "ctrl+a,b"),
            [0x01, 0x00, 0x00, 0x09, 0x00, 0xF0, 0x04, 0xFE, 0x04, 0xFE, 0xF0, 0x05, 0xFE, 0x05]
        );
    }

    #[test]
    fn encode_pedal_c_tap_a() {
        assert_eq!(
            encode_ok("c", "a"),
            [0x03, 0x00, 0x00, 0x01, 0x02, 0x04, 0xFE, 0x04]
        );
        assert_eq!(Pedal::from_string("C").unwrap(), Pedal::C);
        assert_eq!(Pedal::C.as_byte(), 0x03);
    }

    #[test]
    fn encode_every_single_modifier_plus_a() {
        let rows = [
            ("ctrl+a", [0xF0_u8][..].to_vec()),
            ("shift+a", vec![0xF1]),
            ("alt+a", vec![0xF2]),
            ("gui+a", vec![0xF3]),
            ("rctrl+a", vec![0xF4]),
            ("rshift+a", vec![0xF5]),
            ("ralt+a", vec![0xF6]),
            ("rgui+a", vec![0xF7]),
        ];
        for (spelling, tokens) in rows {
            let token = tokens[0];
            assert_eq!(
                encode_ok("a", spelling),
                [0x01, 0x00, 0x00, 0x02, 0x04, token, 0x04, 0xFE, 0x04, 0xFE, token],
                "{spelling}"
            );
        }
    }

    #[test]
    fn encode_sequence_a_b_c_and_shift_a_then_b_and_clear() {
        assert_eq!(
            encode_ok("a", "a,b,c"),
            [0x01, 0x00, 0x00, 0x09, 0x00, 0x04, 0xFE, 0x04, 0x05, 0xFE, 0x05, 0x06, 0xFE, 0x06]
        );
        assert_eq!(
            encode_ok("a", "shift+a,b"),
            [0x01, 0x00, 0x00, 0x09, 0x00, 0xF1, 0x04, 0xFE, 0x04, 0xFE, 0xF1, 0x05, 0xFE, 0x05]
        );
        assert_eq!(encode_ok("a", "clear"), [0x01, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(encode_ok("b", "CLEAR"), [0x02, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(encode_ok("c", "clear"), [0x03, 0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn encode_multi_modifiers_uses_canonical_reverse_release() {
        // Canonical press is F0..F7; release is the reverse of the pressed set.
        // Captured physical release order varied, so those rows are documented
        // here rather than hardcoded:
        //   Ctrl+Shift+A capture FE F1 FE F0 matches canonical reverse.
        //   Ctrl+Alt+A capture FE F0 FE F2 (press order, not reverse).
        //   Shift+Alt+A capture FE F1 FE F2 (press order, not reverse).
        //   Ctrl+Shift+Alt+A capture FE F2 FE F0 FE F1.
        //   Ctrl+Shift+Alt+GUI+A capture FE F3 FE F2 FE F0 FE F1.
        assert_eq!(
            encode_ok("a", "ctrl+shift+a"),
            [0x01, 0x00, 0x00, 0x03, 0x06, 0xF0, 0xF1, 0x04, 0xFE, 0x04, 0xFE, 0xF1, 0xFE, 0xF0]
        );
        assert_eq!(
            encode_ok("a", "ctrl+alt+a"),
            [0x01, 0x00, 0x00, 0x03, 0x06, 0xF0, 0xF2, 0x04, 0xFE, 0x04, 0xFE, 0xF2, 0xFE, 0xF0]
        );
        assert_eq!(
            encode_ok("a", "shift+alt+a"),
            [0x01, 0x00, 0x00, 0x03, 0x06, 0xF1, 0xF2, 0x04, 0xFE, 0x04, 0xFE, 0xF2, 0xFE, 0xF1]
        );
        assert_eq!(
            encode_ok("a", "ctrl+shift+alt+a"),
            [
                0x01, 0x00, 0x00, 0x04, 0x08, 0xF0, 0xF1, 0xF2, 0x04, 0xFE, 0x04, 0xFE, 0xF2, 0xFE,
                0xF1, 0xFE, 0xF0
            ]
        );
        assert_eq!(
            encode_ok("a", "ctrl+shift+alt+gui+a"),
            [
                0x01, 0x00, 0x00, 0x05, 0x0A, 0xF0, 0xF1, 0xF2, 0xF3, 0x04, 0xFE, 0x04, 0xFE, 0xF3,
                0xFE, 0xF2, 0xFE, 0xF1, 0xFE, 0xF0
            ]
        );
        // User order does not change the canonical payload.
        assert_eq!(
            encode_ok("a", "gui+alt+shift+ctrl+a"),
            encode_ok("a", "ctrl+shift+alt+gui+a")
        );
        assert_eq!(
            ProgramAction::from_string("shift+ctrl+a")
                .unwrap()
                .to_string(),
            "ctrl+shift+a"
        );
    }

    #[test]
    fn encode_function_keys_f1_f12_and_rejects_f13_f24() {
        assert_eq!(
            encode_ok("a", "f1"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x3A, 0xFE, 0x3A]
        );
        assert_eq!(
            encode_ok("a", "f12"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x45, 0xFE, 0x45]
        );
        for spelling in [
            "f13",
            "f14",
            "f15",
            "f16",
            "f17",
            "f18",
            "f19",
            "f20",
            "f21",
            "f22",
            "f23",
            "f24",
            "F24",
            "rctrl+f24",
            "ctrl+f13",
            "a,f24",
        ] {
            let err = encode_program_from_str("a", spelling).unwrap_err();
            let message = err.to_string();
            assert!(
                message.contains("Device limitation")
                    && message.contains("F13-F24")
                    && message.contains("no Play event"),
                "{spelling} must fail before USB with a device limitation: {err}"
            );
        }
        assert_eq!(usb_hid::parse_key_name("f13"), Some(usb_hid::KEY_F13));
        assert_eq!(usb_hid::parse_key_name("f24"), Some(usb_hid::KEY_F24));
    }

    #[test]
    fn encode_special_and_keypad_usages() {
        assert_eq!(
            encode_ok("a", "delete"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x4C, 0xFE, 0x4C]
        );
        assert_eq!(
            encode_ok("a", "right"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x4F, 0xFE, 0x4F]
        );
        assert_eq!(
            encode_ok("a", "keypad-enter"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x58, 0xFE, 0x58]
        );
        assert_eq!(
            encode_ok("a", "printscreen"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x46, 0xFE, 0x46]
        );
        assert_eq!(
            encode_ok("a", "comma"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x36, 0xFE, 0x36]
        );
        assert_eq!(
            encode_ok("a", "application"),
            [0x01, 0x00, 0x00, 0x01, 0x02, 0x65, 0xFE, 0x65]
        );
    }

    #[test]
    fn encode_mouse_and_single_modifier_match_capture() {
        assert_eq!(
            encode_ok("a", "left-click"),
            [0x01, 0x20, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            encode_ok("b", "right-click"),
            [0x02, 0x20, 0x00, 0x04, 0x00, 0x02, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            encode_ok("c", "middle-click"),
            [0x03, 0x20, 0x00, 0x04, 0x00, 0x04, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            encode_ok("a", "scroll-up"),
            [0x01, 0x20, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x01]
        );
        assert_eq!(
            encode_ok("b", "scroll-down"),
            [0x02, 0x20, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0xFF]
        );
        assert_eq!(
            encode_ok("c", "ctrl"),
            [0x03, 0x00, 0x00, 0x01, 0x02, 0xF0, 0xFE, 0xF0]
        );
        assert_eq!(encode_ok("a", "leftclick"), encode_ok("a", "left-click"));
        assert_eq!(encode_ok("a", "CTRL"), encode_ok("a", "ctrl"));
        assert_eq!(
            ProgramAction::from_string("ctrl").unwrap(),
            ProgramAction::ModifierOnly(ProgramModifier::LeftCtrl)
        );
    }

    #[test]
    fn program_parser_rejects_mouse_combinations() {
        for spelling in [
            "left-click,a",
            "a,right-click",
            "ctrl+left-click",
            "left-click,right-click",
        ] {
            let err = ProgramAction::from_string(spelling).unwrap_err();
            let message = err.to_string().to_lowercase();
            assert!(
                message.contains("mouse") || message.contains("cannot be combined"),
                "{spelling} should reject combined mouse: {err}"
            );
        }
    }

    #[test]
    fn program_parser_accepts_documented_aliases() {
        let ctrl = encode_ok("a", "control+a");
        assert_eq!(ctrl, encode_ok("a", "lctrl+a"));
        assert_eq!(ctrl, encode_ok("a", "ctrl+a"));

        assert_eq!(encode_ok("a", "lshift+a"), encode_ok("a", "shift+a"));
        assert_eq!(encode_ok("a", "option+a"), encode_ok("a", "alt+a"));
        assert_eq!(encode_ok("a", "lalt+a"), encode_ok("a", "alt+a"));
        assert_eq!(encode_ok("a", "win+a"), encode_ok("a", "gui+a"));
        assert_eq!(encode_ok("a", "cmd+a"), encode_ok("a", "lgui+a"));
        assert_eq!(encode_ok("a", "rwin+a"), encode_ok("a", "rgui+a"));

        assert_eq!(usb_hid::parse_key_name("del"), Some(usb_hid::KEY_DELETE));
        assert_eq!(
            usb_hid::parse_key_name("keypad-plus"),
            Some(usb_hid::KEY_KEYPAD_ADD)
        );
        assert_eq!(
            usb_hid::parse_key_name("page-up"),
            Some(usb_hid::KEY_PAGEUP)
        );
        assert_eq!(
            usb_hid::parse_key_name("menu"),
            Some(usb_hid::KEY_APPLICATION)
        );
    }

    #[test]
    fn program_parser_rejects_empty_modifier_only_malformed_clear_and_unknown() {
        let empty = ProgramAction::from_string("").unwrap_err();
        assert!(
            empty.to_string().contains("cannot be empty"),
            "empty action: {empty}"
        );

        for spelling in ["shift+alt", "ctrl+", "+a", "a+", "ctrl++a"] {
            let err = ProgramAction::from_string(spelling).unwrap_err();
            let message = err.to_string().to_lowercase();
            assert!(
                message.contains("modifier-only") || message.contains("malformed"),
                "{spelling} should be rejected: {err}"
            );
        }

        for spelling in ["clear,a", "a,clear", "clear,clear"] {
            let err = ProgramAction::from_string(spelling).unwrap_err();
            assert!(
                err.to_string().contains("clear"),
                "{spelling} should reject combined clear: {err}"
            );
        }

        for spelling in ["a,", ",a", "a,,b", "notakey", "f25"] {
            let err = ProgramAction::from_string(spelling).unwrap_err();
            let message = err.to_string().to_lowercase();
            assert!(
                message.contains("malformed")
                    || message.contains("unknown key")
                    || message.contains("empty"),
                "{spelling} should be rejected: {err}"
            );
        }
    }

    #[test]
    fn program_parser_rejects_consumer_media_mouse_numeric() {
        for spelling in [
            "play", "volumeup", "mute", "mouse", "click", "power", "sleep", "delay", "repeat",
            "0x04", "04",
        ] {
            let err = encode_program_from_str("a", spelling).unwrap_err();
            let message = err.to_string().to_lowercase();
            assert!(
                message.contains("consumer")
                    || message.contains("media")
                    || message.contains("numeric")
                    || message.contains("out of scope"),
                "{spelling} should be rejected before USB: {err}"
            );
        }
    }

    #[test]
    fn encode_rejects_journal_over_0x800() {
        let too_long = vec!["a"; 700].join(",");
        let err = encode_program_from_str("a", &too_long).unwrap_err();
        assert!(
            err.to_string().contains("0x800"),
            "over-length sequence must fail before USB: {err}"
        );
    }

    #[test]
    fn encode_pedal_b_tap_b_is_now_valid() {
        assert_eq!(
            encode_ok("b", "b"),
            [0x02, 0x00, 0x00, 0x01, 0x02, 0x05, 0xFE, 0x05]
        );
    }

    #[test]
    fn parse_rejects_unknown_pedal() {
        let err = Pedal::from_string("d").unwrap_err();
        assert!(
            err.to_string().contains("Unsupported pedal"),
            "unknown pedal: {err}"
        );
        let empty = Pedal::from_string("").unwrap_err();
        assert!(empty.to_string().contains("cannot be empty"));
    }
}
