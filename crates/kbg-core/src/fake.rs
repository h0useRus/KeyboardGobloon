//! Two fake layouts, US and Russian, for the tests.

use std::cell::Cell;

use crate::{LayoutId, Layouts, Stroke};

pub const EN: LayoutId = LayoutId(0x0409_0409);
pub const RU: LayoutId = LayoutId(0x0419_0419);

/// A digit and the letter rows in US and Russian (no Shift: capitals are the letters' uppercase).
pub const KEYS: &[(u16, char, char)] = &[
    (0x03, '2', '2'),
    (0x29, '`', 'ё'),
    (0x10, 'q', 'й'),
    (0x11, 'w', 'ц'),
    (0x12, 'e', 'у'),
    (0x13, 'r', 'к'),
    (0x14, 't', 'е'),
    (0x15, 'y', 'н'),
    (0x16, 'u', 'г'),
    (0x17, 'i', 'ш'),
    (0x18, 'o', 'щ'),
    (0x19, 'p', 'з'),
    (0x1A, '[', 'х'),
    (0x1B, ']', 'ъ'),
    (0x1E, 'a', 'ф'),
    (0x1F, 's', 'ы'),
    (0x20, 'd', 'в'),
    (0x21, 'f', 'а'),
    (0x22, 'g', 'п'),
    (0x23, 'h', 'р'),
    (0x24, 'j', 'о'),
    (0x25, 'k', 'л'),
    (0x26, 'l', 'д'),
    (0x27, ';', 'ж'),
    (0x28, '\'', 'э'),
    (0x2C, 'z', 'я'),
    (0x2D, 'x', 'ч'),
    (0x2E, 'c', 'с'),
    (0x2F, 'v', 'м'),
    (0x30, 'b', 'и'),
    (0x31, 'n', 'т'),
    (0x32, 'm', 'ь'),
    (0x33, ',', 'б'),
    (0x34, '.', 'ю'),
    (0x35, '/', '.'),
];

/// The keys that type `text` in `layout`.
pub fn strokes(layout: LayoutId, text: &str) -> Vec<Stroke> {
    let fake = Fake::new(layout);
    text.chars()
        .map(|c| fake.stroke(layout, c).expect("a key in KEYS"))
        .collect()
}

pub struct Fake {
    pub current: Cell<LayoutId>,
    pub installed: Vec<LayoutId>,
}

impl Fake {
    pub fn new(current: LayoutId) -> Self {
        Self {
            current: Cell::new(current),
            installed: vec![EN, RU],
        }
    }
}

impl Layouts for Fake {
    fn current(&self) -> Option<LayoutId> {
        Some(self.current.get())
    }

    fn installed(&self) -> Vec<LayoutId> {
        self.installed.clone()
    }

    fn render(&self, layout: LayoutId, strokes: &[Stroke]) -> Option<String> {
        strokes
            .iter()
            .map(|s| {
                let &(_, en, ru) = KEYS.iter().find(|k| k.0 == s.key.scan)?;
                let c = if layout == RU { ru } else { en };
                Some(if s.shift != s.caps {
                    c.to_uppercase().next()?
                } else {
                    c
                })
            })
            .collect()
    }

    fn stroke(&self, layout: LayoutId, c: char) -> Option<Stroke> {
        let lower = c.to_lowercase().next()?;
        let &(scan, ..) = KEYS
            .iter()
            .find(|&&(_, en, ru)| lower == if layout == RU { ru } else { en })?;
        let mut stroke = Stroke::plain(scan);
        stroke.shift = c.is_uppercase();
        Some(stroke)
    }
}
