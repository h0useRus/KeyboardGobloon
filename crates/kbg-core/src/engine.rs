use kbg_settings::Action;

use crate::detect::Detector;
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
    /// A hotkey from the settings.
    Action(Action),
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
    /// Retype the selected text in the next layout ([`crate::convert_text`]).
    ConvertSelection,
    /// Give the selected text the next case ([`crate::cycle_case`]).
    ChangeCase,
}

/// The keyboard layouts, as the platform sees them.
pub trait Layouts {
    /// The layout of the window being typed into.
    fn current(&self) -> Option<LayoutId>;
    /// Every installed layout, in the order the system switches through them.
    fn installed(&self) -> Vec<LayoutId>;
    /// What `strokes` type in `layout`; `None` when a key types nothing there (or a dead key).
    fn render(&self, layout: LayoutId, strokes: &[Stroke]) -> Option<String>;
    /// The key and Shift/AltGr state that type `c` in `layout`; `None` when no key does.
    fn stroke(&self, layout: LayoutId, c: char) -> Option<Stroke>;
}

/// The layout after `current` in the system's order (a layout not in the list counts as before
/// the first); `None` with fewer than two layouts.
pub fn next_layout(layouts: &dyn Layouts, current: LayoutId) -> Option<LayoutId> {
    let installed = layouts.installed();
    let next = installed
        .iter()
        .position(|&l| l == current)
        .map_or(0, |i| (i + 1) % installed.len());
    installed.get(next).copied().filter(|&l| l != current)
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
    /// The layout the word was typed in, when the detector converted it: the hotkey undoes that.
    auto_from: Option<LayoutId>,
    /// Converts words typed in the wrong layout as they end; `None` = auto-switching is off.
    detector: Option<Detector>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            word: Vec::with_capacity(MAX_WORD),
            ..Self::default()
        }
    }

    pub fn set_detector(&mut self, detector: Option<Detector>) {
        self.detector = detector;
    }

    pub fn handle(&mut self, input: Input, layouts: &dyn Layouts) -> Vec<Command> {
        match input {
            Input::Text(stroke) => self.text(stroke),
            Input::Space => {
                if !self.word.is_empty() {
                    self.trailing += 1;
                    if self.trailing == 1 {
                        return self.word_ended(layouts);
                    }
                } else {
                    // The end of a word too long to keep.
                    self.overflow = false;
                }
            }
            Input::Backspace => self.backspace(),
            Input::Break => self.clear(),
            Input::Action(action) => return self.action(action, layouts),
        }
        Vec::new()
    }

    fn action(&mut self, action: Action, layouts: &dyn Layouts) -> Vec<Command> {
        if action == Action::ConvertLast {
            return self.convert(layouts);
        }
        let current = self.shown_in.or_else(|| layouts.current());
        // Every other action leaves the word behind.
        self.clear();
        match action {
            Action::SwitchLayout => current
                .and_then(|c| next_layout(layouts, c))
                .map(Command::SwitchLayout)
                .into_iter()
                .collect(),
            Action::ConvertSelection => vec![Command::ConvertSelection],
            Action::ChangeCase => vec![Command::ChangeCase],
            Action::ConvertLast | Action::ToggleEnabled => Vec::new(),
        }
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
        self.auto_from = None;
    }

    /// The word just got its first space: retype it if the detector says it is in the wrong layout.
    fn word_ended(&mut self, layouts: &dyn Layouts) -> Vec<Command> {
        // A word we converted already, or one whose characters no longer match its keys.
        if self.shown_in.is_some() || self.shown != self.word.len() {
            return Vec::new();
        }
        let (Some(detector), Some(current)) = (&self.detector, layouts.current()) else {
            return Vec::new();
        };
        let Some(target) = detector.detect(&self.word, current, layouts) else {
            return Vec::new();
        };
        let commands = self.convert_to(target, layouts);
        if !commands.is_empty() {
            self.auto_from = Some(current);
        }
        commands
    }

    fn convert(&mut self, layouts: &dyn Layouts) -> Vec<Command> {
        if self.word.is_empty() {
            return Vec::new();
        }
        // Undoing an automatic switch goes back to where the word came from.
        let target = self.auto_from.take().or_else(|| {
            self.shown_in
                .or_else(|| layouts.current())
                .and_then(|c| next_layout(layouts, c))
        });
        match target {
            Some(target) => self.convert_to(target, layouts),
            None => Vec::new(),
        }
    }

    fn convert_to(&mut self, target: LayoutId, layouts: &dyn Layouts) -> Vec<Command> {
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
    use crate::fake::{EN, Fake, KEYS, RU};

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
            e.handle(Input::Action(Action::ConvertLast), &fake),
            [replace(6, "привет"), Command::SwitchLayout(RU)]
        );
    }

    #[test]
    fn keeps_spaces_and_case() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "Ghbdtn  ");
        assert_eq!(
            e.handle(Input::Action(Action::ConvertLast), &fake)[0],
            replace(8, "Привет  ")
        );
    }

    #[test]
    fn second_press_converts_back_before_the_window_switched() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtn");
        e.handle(Input::Action(Action::ConvertLast), &fake);
        // The window still reports EN.
        assert_eq!(
            e.handle(Input::Action(Action::ConvertLast), &fake),
            [replace(6, "ghbdtn"), Command::SwitchLayout(EN)]
        );
    }

    #[test]
    fn only_the_last_word() {
        let fake = Fake::new(RU);
        let mut e = Engine::new();
        typed(&mut e, &fake, "привет ghbdtn");
        assert_eq!(
            e.handle(Input::Action(Action::ConvertLast), &fake)[0],
            replace(6, "ghbdtn")
        );
    }

    #[test]
    fn backspace_edits_the_word() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtnn ");
        e.handle(Input::Backspace, &fake);
        e.handle(Input::Backspace, &fake);
        assert_eq!(
            e.handle(Input::Action(Action::ConvertLast), &fake)[0],
            replace(6, "привет")
        );
    }

    #[test]
    fn nothing_after_break_or_into_unknown_text() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtn");
        e.handle(Input::Break, &fake);
        assert!(
            e.handle(Input::Action(Action::ConvertLast), &fake)
                .is_empty()
        );

        typed(&mut e, &fake, "g");
        e.handle(Input::Backspace, &fake);
        e.handle(Input::Backspace, &fake);
        assert!(
            e.handle(Input::Action(Action::ConvertLast), &fake)
                .is_empty()
        );
    }

    #[test]
    fn needs_two_layouts() {
        let mut fake = Fake::new(EN);
        fake.installed = vec![EN];
        let mut e = Engine::new();
        typed(&mut e, &fake, "ghbdtn");
        assert!(
            e.handle(Input::Action(Action::ConvertLast), &fake)
                .is_empty()
        );
    }

    #[test]
    fn long_words_are_left_alone() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        typed(&mut e, &fake, &"g".repeat(MAX_WORD + 1));
        assert!(
            e.handle(Input::Action(Action::ConvertLast), &fake)
                .is_empty()
        );
        typed(&mut e, &fake, " ghbdtn");
        assert_eq!(
            e.handle(Input::Action(Action::ConvertLast), &fake)[0],
            replace(6, "привет")
        );
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;
    use crate::fake::{EN, Fake, RU};

    #[test]
    fn switch_layout_goes_to_the_next_one() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        assert_eq!(
            e.handle(Input::Action(Action::SwitchLayout), &fake),
            [Command::SwitchLayout(RU)]
        );
    }

    #[test]
    fn selection_actions_reach_the_platform() {
        let fake = Fake::new(EN);
        let mut e = Engine::new();
        assert_eq!(
            e.handle(Input::Action(Action::ConvertSelection), &fake),
            [Command::ConvertSelection]
        );
        assert_eq!(
            e.handle(Input::Action(Action::ChangeCase), &fake),
            [Command::ChangeCase]
        );
    }
}

#[cfg(test)]
mod detector_tests {
    use super::*;
    use crate::detect::tests::detector;
    use crate::fake::{EN, Fake, RU, strokes};

    fn typed(engine: &mut Engine, layouts: &Fake, text: &str) -> Vec<Command> {
        let mut out = Vec::new();
        for c in text.chars() {
            let input = match c {
                ' ' => Input::Space,
                _ => Input::Text(strokes(layouts.current.get(), &c.to_string())[0]),
            };
            out = engine.handle(input, layouts);
        }
        out
    }

    fn engine() -> Engine {
        let mut e = Engine::new();
        e.set_detector(Some(detector()));
        e
    }

    #[test]
    fn converts_a_wrong_layout_word_at_its_space() {
        let fake = Fake::new(EN);
        let mut e = engine();
        assert!(typed(&mut e, &fake, "ghbdtn").is_empty());
        assert_eq!(
            typed(&mut e, &fake, " "),
            [
                Command::Replace {
                    erase: 7,
                    text: "привет ".into()
                },
                Command::SwitchLayout(RU)
            ]
        );
        // More spaces do not convert again.
        assert!(typed(&mut e, &fake, " ").is_empty());
    }

    #[test]
    fn the_hotkey_undoes_an_automatic_switch() {
        let mut fake = Fake::new(EN);
        // A third layout: undo must go back to EN, not on to the next one.
        fake.installed = vec![EN, RU, crate::LayoutId(0x0407_0407)];
        let mut e = engine();
        typed(&mut e, &fake, "ghbdtn ");
        assert_eq!(
            e.handle(Input::Action(Action::ConvertLast), &fake),
            [
                Command::Replace {
                    erase: 7,
                    text: "ghbdtn ".into()
                },
                Command::SwitchLayout(EN)
            ]
        );
    }

    #[test]
    fn right_words_and_switched_off_detection_stay() {
        let fake = Fake::new(EN);
        let mut e = engine();
        assert!(typed(&mut e, &fake, "hello ").is_empty());
        e.set_detector(None);
        assert!(typed(&mut e, &fake, "ghbdtn ").is_empty());
    }

    #[test]
    fn a_word_converted_by_hand_is_not_judged_again() {
        let fake = Fake::new(RU);
        let mut e = engine();
        // руддщ is hello typed in RU; the hotkey made it hello, and the window still says RU.
        typed(&mut e, &fake, "руддщ");
        e.handle(Input::Action(Action::ConvertLast), &fake);
        assert!(typed(&mut e, &fake, " ").is_empty());
    }
}
