//! Word lists -> `words.fst`: every word form mapped to its frequency on the Zipf scale x 10
//! (log10 of uses per billion words, so packs of different sizes and languages compare);
//! 0 = the source gave no count for that word.

use std::collections::BTreeMap;

/// Lowercase word forms with their summed counts (0 = no count in any source).
pub type Counts = BTreeMap<String, u64>;

/// Adds the lines of one word list: `word` or `word count`, `#` starts a comment. Words are
/// lowercased; one with a letter outside `alphabet` is skipped. Returns the skipped lines.
pub fn parse(text: &str, alphabet: &[char], counts: &mut Counts) -> usize {
    let mut skipped = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let word = parts.next().unwrap_or_default().to_lowercase();
        let count = match (parts.next(), parts.next()) {
            (None, _) => Some(0),
            (Some(n), None) => n.parse::<u64>().ok(),
            _ => None,
        };
        match count {
            Some(n) if word.chars().all(|c| alphabet.contains(&c)) => {
                *counts.entry(word).or_default() += n
            }
            _ => skipped += 1,
        }
    }
    skipped
}

/// Each word's frequency as stored in `words.fst`.
pub fn zipf(counts: &Counts) -> BTreeMap<&str, u64> {
    let total = counts.values().sum::<u64>().max(1) as f64;
    counts
        .iter()
        .map(|(word, &n)| {
            let value = if n == 0 {
                0
            } else {
                ((n as f64 * 1e9 / total).log10() * 10.0).round().max(1.0) as u64
            };
            (word.as_str(), value)
        })
        .collect()
}

/// The `words.fst` bytes (an `fst::Map`, keys in byte order).
pub fn fst(zipf: &BTreeMap<&str, u64>) -> Vec<u8> {
    let mut map = fst::MapBuilder::memory();
    for (word, &value) in zipf {
        map.insert(word, value)
            .expect("BTreeMap keys are sorted and unique");
    }
    map.into_inner().expect("writing to memory")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &[char] = &['a', 'c', 'd', 'g', 'o', 't', 'h', 'e', 'n', '\''];

    #[test]
    fn parses_counts_comments_and_foreign_letters() {
        let mut counts = Counts::new();
        let text =
            "# list\nthe 600\nCat 30\n\ndog\ncat 10\nzebra 5\ndon't 2\nbad line here\nego x\n";
        assert_eq!(parse(text, EN, &mut counts), 3);
        assert_eq!(counts["the"], 600);
        assert_eq!(counts["cat"], 40);
        assert_eq!(counts["dog"], 0);
        assert_eq!(counts["don't"], 2);
        assert!(!counts.contains_key("zebra"));
    }

    #[test]
    fn zipf_ranks_frequent_words_higher() {
        let counts = Counts::from([
            ("the".into(), 999_000),
            ("cat".into(), 1_000),
            ("dog".into(), 0),
        ]);
        let z = zipf(&counts);
        assert!(z["the"] > z["cat"]);
        assert_eq!(z["cat"], 60);
        assert_eq!(z["dog"], 0);
    }

    #[test]
    fn fst_maps_words_to_frequencies() {
        let counts = Counts::from([("кот".into(), 5), ("the".into(), 5)]);
        let map = fst::Map::new(fst(&zipf(&counts))).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("кот"), Some(87));
        assert_eq!(map.get("кит"), None);
    }
}
