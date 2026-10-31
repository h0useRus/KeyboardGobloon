//! Hotkeys: modifiers plus one key, written as text like `Shift+Pause` in settings.gob.
//!
//! Keys are Windows virtual-key codes. A hotkey whose key is itself a modifier (`RCtrl`) with no
//! other modifiers is a tap: it fires when that key is pressed and released alone.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Virtual-key code.
    pub key: u16,
}

/// Names of the keys, as written in settings.gob and shown in the settings window. Letters and
/// digits are added in [`key_name`] / [`key_code`].
const NAMES: &[(u16, &str)] = &[
    (0x08, "Backspace"),
    (0x09, "Tab"),
    (0x0D, "Enter"),
    (0x13, "Pause"),
    (0x14, "CapsLock"),
    (0x1B, "Esc"),
    (0x20, "Space"),
    (0x21, "PageUp"),
    (0x22, "PageDown"),
    (0x23, "End"),
    (0x24, "Home"),
    (0x25, "Left"),
    (0x26, "Up"),
    (0x27, "Right"),
    (0x28, "Down"),
    (0x2C, "PrintScreen"),
    (0x2D, "Insert"),
    (0x2E, "Delete"),
    (0x5B, "LWin"),
    (0x5C, "RWin"),
    (0x5D, "Menu"),
    (0x90, "NumLock"),
    (0x91, "ScrollLock"),
    (0xA0, "LShift"),
    (0xA1, "RShift"),
    (0xA2, "LCtrl"),
    (0xA3, "RCtrl"),
    (0xA4, "LAlt"),
    (0xA5, "RAlt"),
    (0xBA, ";"),
    (0xBB, "="),
    (0xBC, ","),
    (0xBD, "-"),
    (0xBE, "."),
    (0xBF, "/"),
    (0xC0, "`"),
    (0xDB, "["),
    (0xDC, "\\"),
    (0xDD, "]"),
    (0xDE, "'"),
];

/// The modifier keys, left and right (also as taps).
pub fn is_modifier(key: u16) -> bool {
    matches!(key, 0x5B | 0x5C | 0xA0..=0xA5)
}

/// `Pause`, `A`, `F5`, `Num5`; unknown keys as `VK_E2`.
pub fn key_name(key: u16) -> String {
    match key {
        0x30..=0x39 | 0x41..=0x5A => char::from(key as u8).to_string(),
        0x60..=0x69 => format!("Num{}", key - 0x60),
        0x70..=0x87 => format!("F{}", key - 0x6F),
        _ => NAMES
            .iter()
            .find(|(k, _)| *k == key)
            .map_or_else(|| format!("VK_{key:02X}"), |(_, n)| n.to_string()),
    }
}

/// The key named `name` (as [`key_name`] writes it, any case).
pub fn key_code(name: &str) -> Option<u16> {
    let upper = name.to_ascii_uppercase();
    if let [c] = upper.as_bytes()
        && c.is_ascii_alphanumeric()
    {
        return Some(u16::from(*c));
    }
    if let Some(n) = upper
        .strip_prefix("NUM")
        .and_then(|n| n.parse::<u16>().ok())
        && n <= 9
    {
        return Some(0x60 + n);
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u16>().ok())
        && (1..=24).contains(&n)
    {
        return Some(0x6F + n);
    }
    if let Some(hex) = upper.strip_prefix("VK_") {
        return u16::from_str_radix(hex, 16)
            .ok()
            .filter(|&k| k > 0 && k < 0xFF);
    }
    NAMES
        .iter()
        .find(|(_, n)| n.eq_ignore_ascii_case(name))
        .map(|(k, _)| *k)
}

impl Hotkey {
    pub const fn key(key: u16) -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            key,
        }
    }

    pub const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// A modifier key on its own: pressed and released with nothing else.
    pub fn is_tap(&self) -> bool {
        is_modifier(self.key) && !(self.ctrl || self.alt || self.shift || self.win)
    }

    /// From keys pressed together (virtual-key codes, in any order): the modifiers among them
    /// and the one other key; a single modifier is a tap. `None` for two non-modifier keys.
    pub fn from_keys(keys: &[u16]) -> Option<Self> {
        let others: Vec<u16> = keys.iter().copied().filter(|&k| !is_modifier(k)).collect();
        match others.as_slice() {
            [] => match keys {
                [only] => Some(Self::key(*only)),
                _ => None,
            },
            [key] => Some(Self {
                ctrl: keys.iter().any(|&k| matches!(k, 0xA2 | 0xA3)),
                alt: keys.iter().any(|&k| matches!(k, 0xA4 | 0xA5)),
                shift: keys.iter().any(|&k| matches!(k, 0xA0 | 0xA1)),
                win: keys.iter().any(|&k| matches!(k, 0x5B | 0x5C)),
                key: *key,
            }),
            _ => None,
        }
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.win, "Win"),
        ] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&key_name(self.key))
    }
}

impl std::str::FromStr for Hotkey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let mut hotkey = Self::default();
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        // `Ctrl++` would be the plus key: not supported, keys have names.
        let Some((last, mods)) = parts.split_last() else {
            return Err(format!("empty hotkey: {s:?}"));
        };
        for m in mods {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" => hotkey.ctrl = true,
                "alt" => hotkey.alt = true,
                "shift" => hotkey.shift = true,
                "win" => hotkey.win = true,
                _ => return Err(format!("unknown modifier {m:?} in {s:?}")),
            }
        }
        hotkey.key = key_code(last).ok_or_else(|| format!("unknown key {last:?} in {s:?}"))?;
        Ok(hotkey)
    }
}

/// Written as text; an empty string means no hotkey (see `Option<Hotkey>` in the settings).
pub(crate) mod text {
    use super::*;

    pub fn serialize<S: Serializer>(hotkey: &Option<Hotkey>, s: S) -> Result<S::Ok, S::Error> {
        hotkey
            .map(|h| h.to_string())
            .unwrap_or_default()
            .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Hotkey>, D::Error> {
        let text = String::deserialize(d)?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        text.parse().map(Some).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trip() {
        for text in [
            "Pause",
            "Shift+Pause",
            "Ctrl+Alt+K",
            "RCtrl",
            "Win+F12",
            "Num5",
            "VK_E2",
        ] {
            let hotkey: Hotkey = text.parse().unwrap();
            assert_eq!(hotkey.to_string(), text);
        }
        assert_eq!(
            "shift + pause".parse::<Hotkey>().unwrap(),
            Hotkey::key(0x13).shift()
        );
        assert!("Hyper+A".parse::<Hotkey>().is_err());
        assert!("Shift+Nope".parse::<Hotkey>().is_err());
    }

    #[test]
    fn taps() {
        assert!("RCtrl".parse::<Hotkey>().unwrap().is_tap());
        assert!(!"Shift+RCtrl".parse::<Hotkey>().unwrap().is_tap());
        assert!(!"CapsLock".parse::<Hotkey>().unwrap().is_tap());
    }

    #[test]
    fn from_pressed_keys() {
        // LShift + Pause
        assert_eq!(
            Hotkey::from_keys(&[0xA0, 0x13]),
            Some(Hotkey::key(0x13).shift())
        );
        // Right Ctrl alone: a tap.
        assert_eq!(Hotkey::from_keys(&[0xA3]), Some(Hotkey::key(0xA3)));
        // Two modifiers and no key, or two keys: not a hotkey.
        assert_eq!(Hotkey::from_keys(&[0xA0, 0xA2]), None);
        assert_eq!(Hotkey::from_keys(&[0x41, 0x42]), None);
    }
}
