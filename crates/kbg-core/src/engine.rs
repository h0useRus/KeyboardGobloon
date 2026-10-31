use crate::keys::{LayoutId, Stroke};

/// Longest word kept; longer ones are not converted (nobody mistypes a whole line as one word).
const MAX_WORD: usize = 64;

/// What the platform reports from the keyboard and mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// A layout-dependent character key.
    Text(Stroke),
    Space,
    Backspace,
    /// The caret may have moved or the word context is gone: Enter, Tab, arrows, a shortcut,
    /// a mouse click, another window...
    Break,
    /// Hotkey: show the last word in the next layout and switch to it.
    ConvertLast,
}

/// What the platform must do, in this order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Delete `erase` characters before the caret, then type `text`.
    Replace {
        erase: usize,
        text: String,
    },
    SwitchLayout(LayoutId),
}

/// The keyboard layouts, as the platform sees them.
pub trait Layouts {
    /// The layout of the window being typed into.
    fn current(&self) -> Option<LayoutId>;
    /// Every installed layout, in the order the system switches through them.
    fn installed(&self) -> Vec<LayoutId>;
    /// What `strokes` type in `layout`; `None` when a key types nothing there (or a dead key).
    fn render(&self, layout: LayoutId, strokes: &[Stroke]) -> Option<String>;
}

/// The last word typed, as keys, and the spaces typed after it.
#[derive(Default)]
pub struct Engine {
    word: Vec<Stroke>,
    /// Characters of the word on screen (one per stroke until a conversion says otherwise).
    shown: usize,
    trailing: usize,
    /// Too long to keep: ignored until the next word.
    overflow: bool,
    /// The layout the word is shown in since we converted it; the system may not have
    /// switched the window yet when the hotkey comes again.
    shown_in: Option<LayoutId>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            word: Vec::with_capacity(MAX_WORD),
            ..Self::default()
        }
    }

    pub fn handle(&mut self, input: Input, layouts: &dyn Layouts) -> Vec<Command> {
        match input {
            Input::Text(stroke) => self.text(stroke),
            Input::Space => {
                if !self.word.is_empty() {
                    self.trailing += 1;
                } else {
                    // The end of a word too long to keep.
                    self.overflow = false;
                }
            }
            Input::Backspace => self.backspace(),
            Input::Break => self.clear(),
            Input::ConvertLast => return self.convert(layouts),
        }
        Vec::new()
    }

    fn text(&mut self, stroke: Stroke) {
        if self.trailing > 0 {
            self.clear();
        }
        if self.overflow {
            return;
        }
        if self.word.len() == MAX_WORD {
            self.clear();
            self.overflow = true;
            return;
        }
        self.word.push(stroke);
        self.shown += 1;
    }

    fn backspace(&mut self) {
        if self.trailing > 0 {
            self.trailing -= 1;
        } else if self.overflow || self.shown != self.word.len() {
            // Inside a word we don't know, or one whose characters no longer match its keys.
            self.clear();
        } else if self.word.pop().is_some() {
            self.shown -= 1;
            if self.word.is_empty() {
                self.clear();
            }
        }
    }

    fn clear(&mut self) {
        self.word.clear();
        self.shown = 0;
        self.trailing = 0;
        self.overflow = false;
        self.shown_in = None;
    }

    fn convert(&mut self, layouts: &dyn Layouts) -> Vec<Command> {
        if self.word.is_empty() {
            return Vec::new();
        }
        let installed = layouts.installed();
        let Some(current) = self.shown_in.or_else(|| layouts.current()) else {
            return Vec::new();
        };
        // The next layout after the current one; a current layout not in the list counts as
        // before the first.
        let next = installed
            .iter()
            .position(|&l| l == current)
            .map_or(0, |i| (i + 1) % installed.len());
        let Some(&target) = installed.get(next).filter(|&&l| l != current) else {
            return Vec::new();
        };
        let Some(word) = layouts.render(target, &self.word) else {
            return Vec::new();
        };
        let erase = self.shown + self.trailing;
        self.shown = word.chars().count();
        self.shown_in = Some(target);
        let text = word + &" ".repeat(self.trailing);
        vec![
            Command::Replace { erase, text },
            Command::SwitchLayout(target),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const EN: LayoutId = LayoutId(0x0409_0409);
    const RU: LayoutId = LayoutId(0x0419_0419);

    /// The keys of `ghbdtn` / `привет` and a few others, in US and Russian.
    const KEYS: &[(u16, char, char)] = &[
        (0x22, 'g', 'п'),
        (0x23, 'h', 'р'),
        (0x30, 'b', 'и'),
        (0x20, 'd', 'в'),
        (0x14, 't', 'е'),
        (0x31, 'n', 'т'),
        (0x1A, '[', 'х'),
    ];

    struct Fake {
        current: Cell<LayoutId>,
        installed: Vec<LayoutId>,
    }

    impl Fake {
        fn new(current: LayoutId) -> Self {
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
    }

    fn typed(engine: &mut Engine, layouts: &Fake, word: &str) {
        for c in word.chars() {
            let input = if c == ' ' {
                Input::Space
            } else {
                let lower = c.to_lowercase().next().unwrap();
                let key = KEYS
                    .iter()
                    .find(|k| k.1 == lower || k.2 == lower)
                    .expect("key in KEYS");
                let mut stroke = Stroke::plain(key.0);
                stroke.shift = c.is_uppercase();
                Input::Text(stroke)
            };
            engine.handle(input, layouts);
        }
    }

    fn replace(erase: usize, text: &str) -> Command {
        Command::Replace {
            erase,
            text: text.into(),
        }
    }

    #[test]
    fn converts_last_word() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtn");
        assert_eq!(
            e.handle(Input::ConvertLast, &fake),
            [replace(6, "привет"), Command::SwitchLayout(RU)]
        );
    }

    #[test]
    fn keeps_spaces_and_case() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "Ghbdtn  ");
        assert_eq!(
            e.handle(Input::ConvertLast, &fake)[0],
            replace(8, "Привет  ")
        );
    }

    #[test]
    fn second_press_converts_back_before_the_window_switched() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtn");
        e.handle(Input::ConvertLast, &fake);
        // The window still reports EN.
        assert_eq!(
            e.handle(Input::ConvertLast, &fake),
            [replace(6, "ghbdtn"), Command::SwitchLayout(EN)]
        );
    }

    #[test]
    fn only_the_last_word() {
        let fake = Fake::new(RU);
        let mut e = Engine::new();
        typed(&mut e, &fake, "привет ghbdtn");
        assert_eq!(e.handle(Input::ConvertLast, &fake)[0], replace(6, "ghbdtn"));
    }

    #[test]
    fn backspace_edits_the_word() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtnn ");
        e.handle(Input::Backspace, &fake);
        e.handle(Input::Backspace, &fake);
        assert_eq!(e.handle(Input::ConvertLast, &fake)[0], replace(6, "привет"));
    }

    #[test]
    fn nothing_after_break_or_into_unknown_text() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtn");
        e.handle(Input::Break, &fake);
        assert!(e.handle(Input::ConvertLast, &fake).is_empty());

        typed(&mut e, &fake, "g");
        e.handle(Input::Backspace, &fake);
        e.handle(Input::Backspace, &fake);
        assert!(e.handle(Input::ConvertLast, &fake).is_empty());
    }

    #[test]
    fn needs_two_layouts() {
        let mut fake = Fake::new(EN);
        fake.installed = vec![EN];
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtn");
        assert!(e.handle(Input::ConvertLast, &fake).is_empty());
    }

    #[test]
    fn long_words_are_left_alone() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, &"g".repeat(MAX_WORD + 1));
        assert!(e.handle(Input::ConvertLast, &fake).is_empty());
        typed(&mut e, &fake, " ghbdtn");
        assert_eq!(e.handle(Input::ConvertLast, &fake)[0], replace(6, "привет"));
    }
}
