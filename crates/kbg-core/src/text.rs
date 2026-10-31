//! Changes to selected text: retyping it in another layout, cycling its case.

use crate::engine::{Layouts, next_layout};
use crate::keys::LayoutId;

/// Retypes text typed in the wrong layout. The layout it was typed in is the one whose keys
/// type the most of its letters (the window's current layout wins a tie); the same keys are then
/// shown in the next layout. Characters that are not on a letter, digit or punctuation key
/// there (spaces, line breaks, emoji) stay as they are. Returns the new text and its layout;
/// `None` when nothing would change.
pub fn convert_text(text: &str, layouts: &dyn Layouts) -> Option<(String, LayoutId)> {
    let current = layouts.current();
    let candidates = current.into_iter().chain(
        layouts
            .installed()
            .into_iter()
            .filter(|&l| Some(l) != current),
    );
    let typed_in = |layout| {
        text.chars()
            .filter(|c| c.is_alphabetic() && layouts.stroke(layout, *c).is_some())
            .count()
    };
    let mut source = None;
    let mut best = 0;
    for layout in candidates {
        let n = typed_in(layout);
        // Strictly more: the current layout, tried first, keeps ties.
        if source.is_none() || n > best {
            source = Some(layout);
            best = n;
        }
    }
    let source = source?;
    let target = next_layout(layouts, source)?;
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        let converted = layouts
            .stroke(source, c)
            .filter(|s| s.key.is_text())
            .and_then(|s| layouts.render(target, &[s]));
        match converted {
            Some(t) => out.push_str(&t),
            None => out.push(c),
        }
    }
    (out != text).then_some((out, target))
}

/// Every word with a capital first letter and the rest small.
fn title(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut start = true;
    for c in text.chars() {
        if start {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
        start = !c.is_alphanumeric();
    }
    out
}

/// The next case in the cycle lower → UPPER → Title → lower; mixed text starts with lower.
/// Cases that look the same as the text (a lone capital is both UPPER and Title) are skipped.
pub fn cycle_case(text: &str) -> String {
    let lower = text.to_lowercase();
    let upper = text.to_uppercase();
    let titled = title(text);
    let cycle = [&lower, &upper, &titled];
    let at = if text == lower {
        0
    } else if text == upper {
        1
    } else if text == titled {
        2
    } else {
        return lower;
    };
    (1..=3)
        .map(|step| cycle[(at + step) % 3])
        .find(|next| next.as_str() != text)
        .cloned()
        .unwrap_or(lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{EN, Fake, RU};

    #[test]
    fn converts_selected_text_from_its_own_layout() {
        // Typed with EN on, but it is Russian.
        let fake = Fake::new(EN);
        assert_eq!(
            convert_text("Ghbdtn ghbdtn!", &fake),
            Some(("Привет привет!".to_string(), RU))
        );
        // The window is in EN, but the text is Russian: it came from RU and goes to EN.
        assert_eq!(
            convert_text("привет", &fake),
            Some(("ghbdtn".to_string(), EN))
        );
    }

    #[test]
    fn nothing_to_convert() {
        let fake = Fake::new(EN);
        assert_eq!(convert_text("  \n", &fake), None);
    }

    #[test]
    fn case_cycle() {
        assert_eq!(cycle_case("hello world"), "HELLO WORLD");
        assert_eq!(cycle_case("HELLO WORLD"), "Hello World");
        assert_eq!(cycle_case("Hello World"), "hello world");
        assert_eq!(cycle_case("hELLO"), "hello");
        assert_eq!(cycle_case("привет"), "ПРИВЕТ");
        // A lone capital is UPPER and Title at once: on to lower.
        assert_eq!(cycle_case("A"), "a");
        assert_eq!(cycle_case("a"), "A");
    }
}
