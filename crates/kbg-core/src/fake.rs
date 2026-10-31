//! Two fake layouts, US and Russian, for the tests.

use std::cell::Cell;

use crate::{LayoutId, Layouts, Stroke};

pub const EN: LayoutId = LayoutId(0x0409_0409);
pub const RU: LayoutId = LayoutId(0x0419_0419);

/// The keys of `ghbdtn` / `привет` and a few others, in US and Russian.
pub const KEYS: &[(u16, char, char)] = &[
    (0x22, 'g', 'п'),
    (0x23, 'h', 'р'),
    (0x30, 'b', 'и'),
    (0x20, 'd', 'в'),
    (0x14, 't', 'е'),
    (0x31, 'n', 'т'),
    (0x1A, '[', 'х'),
];

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
