//! Wrong-layout detection: the last word's keys are shown in every layout that has a language
//! pack, and each rendering is scored by its pack (a dictionary word, or how familiar its letter
//! runs are). The detector only answers when another layout clearly wins; a word it is unsure
//! about stays as typed. The thresholds are first guesses until a test corpus tunes them.

use std::sync::Arc;

use crate::engine::Layouts;
use crate::keys::{LayoutId, Stroke};
use crate::pack::Pack;

/// Shorter words are too ambiguous to judge.
const MIN_LETTERS: usize = 3;
/// When neither rendering is a dictionary word, the other one needs this many letters...
const MIN_LETTERS_UNKNOWN: usize = 4;
/// ... must look this much more like its language (bits per letter) ...
const MARGIN: f32 = 2.5;
/// ... and must look like a word at all.
const MAX_COST: f32 = 6.0;

pub struct Detector {
    packs: Vec<(LayoutId, Arc<Pack>)>,
}

/// How a rendering fits a pack.
struct Score {
    /// A dictionary word.
    hit: bool,
    /// Bits per letter; infinite when a letter is not in the alphabet.
    cost: f32,
    letters: usize,
}

impl Detector {
    /// `packs`: the installed layouts that have a language pack.
    pub fn new(packs: Vec<(LayoutId, Arc<Pack>)>) -> Self {
        Self { packs }
    }

    /// The layout `word` was meant to be typed in, when it is not `current`.
    pub fn detect(
        &self,
        word: &[Stroke],
        current: LayoutId,
        layouts: &dyn Layouts,
    ) -> Option<LayoutId> {
        // Without a pack for the current layout there is nothing to compare against.
        let (_, pack) = self.packs.iter().find(|(l, _)| *l == current)?;
        let typed = layouts.render(current, word)?;
        if typed.chars().any(|c| c.is_ascii_digit()) || camel_case(&typed) {
            return None;
        }
        let here = score(pack, &typed);
        if here.hit {
            return None;
        }
        let mut best: Option<(LayoutId, Score)> = None;
        for (layout, pack) in self.packs.iter().filter(|(l, _)| *l != current) {
            let Some(text) = layouts.render(*layout, word) else {
                continue;
            };
            let other = score(pack, &text);
            let better = match &best {
                None => true,
                Some((_, b)) => (other.hit, -other.cost) > (b.hit, -b.cost),
            };
            if other.letters >= MIN_LETTERS && other.cost.is_finite() && better {
                best = Some((*layout, other));
            }
        }
        let (layout, other) = best?;
        let wins = if other.hit {
            other.cost < here.cost
        } else {
            other.letters >= MIN_LETTERS_UNKNOWN
                && other.cost <= MAX_COST
                && here.cost - other.cost >= MARGIN
        };
        wins.then_some(layout)
    }
}

/// Scores the letters of `text` (punctuation around them is left out).
fn score(pack: &Pack, text: &str) -> Score {
    let core = text
        .trim_matches(|c: char| !c.is_alphabetic())
        .to_lowercase();
    let cost = pack.letter_cost(&core).unwrap_or(f32::INFINITY);
    Score {
        hit: cost.is_finite() && pack.frequency(&core).is_some(),
        cost,
        letters: core.chars().count(),
    }
}

/// `camelCase` or `snake_Case`-like identifiers: a capital after a small letter.
fn camel_case(text: &str) -> bool {
    let mut lower = false;
    text.chars().any(|c| {
        let camel = lower && c.is_uppercase();
        lower = c.is_lowercase();
        camel
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fake::{EN, Fake, RU, strokes};
    use crate::pack::tests::{EN_LETTERS, RU_LETTERS, pack};

    pub fn detector() -> Detector {
        let en = pack(
            "en",
            EN_LETTERS,
            "the 900\nhello 50\nthere 40\nworld 30\nother 20\nhouse 20\nmother 10\nnothing 10\ncat 10\n",
        );
        let ru = pack(
            "ru",
            RU_LETTERS,
            "привет 50\nмир 40\nкот 30\nлюблю 20\nдом 20\nпривета 10\nприветы 10\nкотик 10\nкотики 10\nработа 10\n",
        );
        Detector::new(vec![(EN, Arc::new(en)), (RU, Arc::new(ru))])
    }

    fn detect(current: crate::LayoutId, typed: &str) -> Option<crate::LayoutId> {
        let fake = Fake::new(current);
        detector().detect(&strokes(current, typed), current, &fake)
    }

    #[test]
    fn dictionary_words_in_the_wrong_layout() {
        assert_eq!(detect(EN, "ghbdtn"), Some(RU));
        // Привет. (the RU full stop is the US slash key)
        assert_eq!(detect(EN, "Ghbdtn/"), Some(RU));
        assert_eq!(detect(EN, "rjn"), Some(RU));
        // Russian letters on the punctuation keys of the US layout.
        assert_eq!(detect(EN, "k.,k."), Some(RU));
        assert_eq!(detect(RU, "руддщ"), Some(EN));
    }

    #[test]
    fn right_words_stay() {
        assert_eq!(detect(EN, "hello"), None);
        assert_eq!(detect(RU, "привет"), None);
        assert_eq!(detect(RU, "кот"), None);
    }

    #[test]
    fn unknown_words_by_their_letters() {
        // приветик: not in the dictionary, but made of Russian letter runs.
        assert_eq!(detect(EN, "ghbdtnbr"), Some(RU));
        // Not a word in either language.
        assert_eq!(detect(EN, "qwzx"), None);
    }

    #[test]
    fn short_numeric_and_code_words_stay() {
        assert_eq!(detect(EN, "lf"), None); // да
        assert_eq!(detect(EN, "ghbdtn2"), None);
        assert_eq!(detect(EN, "ghbDtn"), None);
    }

    #[test]
    fn layouts_without_a_pack_are_never_left() {
        let fake = Fake::new(EN);
        let ru_only = Detector::new(
            detector()
                .packs
                .into_iter()
                .filter(|(l, _)| *l == RU)
                .collect(),
        );
        assert_eq!(ru_only.detect(&strokes(EN, "ghbdtn"), EN, &fake), None);
    }
}
