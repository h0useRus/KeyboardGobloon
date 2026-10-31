//! The letter model, `trigrams.bin`: how likely each letter is after the two before it, so a
//! word missing from the dictionary still scores as "looks like this language" or not.
//!
//! Format (little-endian):
//! - `MAGIC`, `u16` `FORMAT`, `u16` n = letters + 1
//! - `u32` length + the alphabet as UTF-8; symbol 0 is the word boundary, symbol i the alphabet's
//!   i-th letter
//! - n * n * n `u8` costs, index `(a * n + b) * n + c`: -log2 P(c | a b) * `SCALE`, at most 255.
//!   A word is scored over `0 0 w1 .. wk 0`, so the first letters are costed from the boundary
//!   and the last cost is the word ending there.

use std::collections::BTreeMap;

pub const MAGIC: &[u8; 4] = b"KBGT";
pub const FORMAT: u16 = 1;
/// Cost units per bit.
pub const SCALE: f64 = 8.0;

/// Each order keeps this share for itself and hands the rest down to the shorter context,
/// so an unseen trigram still costs what its bigram or letter is worth.
const OWN: f64 = 0.7;
/// The share of the letter frequencies that is spread evenly over all symbols.
const FLAT: f64 = 0.1;

/// `zipf` maps each word to its stored frequency (see `words::zipf`). Every word counts
/// 1 + zipf / 10, so frequent words weigh more without drowning the rare forms.
pub fn build(alphabet: &[char], zipf: &BTreeMap<&str, u64>) -> Vec<u8> {
    let n = alphabet.len() + 1;
    let mut uni = vec![0.0; n];
    let mut bi = vec![0.0; n * n];
    let mut tri = vec![0.0; n * n * n];
    for (word, &z) in zipf {
        let weight = 1.0 + z as f64 / 10.0;
        let mut seq = vec![0, 0];
        seq.extend(word.chars().map(|c| symbol(alphabet, c)));
        seq.push(0);
        for w in seq.windows(3) {
            uni[w[2]] += weight;
            bi[w[1] * n + w[2]] += weight;
            tri[(w[0] * n + w[1]) * n + w[2]] += weight;
        }
    }

    let total: f64 = uni.iter().sum();
    let mut out = Vec::with_capacity(12 + 4 * n + n * n * n);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT.to_le_bytes());
    out.extend_from_slice(&(n as u16).to_le_bytes());
    let letters: String = alphabet.iter().collect();
    out.extend_from_slice(&(letters.len() as u32).to_le_bytes());
    out.extend_from_slice(letters.as_bytes());
    for a in 0..n {
        for b in 0..n {
            let bi_sum: f64 = bi[b * n..(b + 1) * n].iter().sum();
            let tri_sum: f64 = tri[(a * n + b) * n..(a * n + b + 1) * n].iter().sum();
            for c in 0..n {
                let p1 = (1.0 - FLAT) * uni[c] / total + FLAT / n as f64;
                let p2 = mix(bi[b * n + c], bi_sum, p1);
                let p3 = mix(tri[(a * n + b) * n + c], tri_sum, p2);
                out.push((-p3.log2() * SCALE).round().min(255.0) as u8);
            }
        }
    }
    out
}

fn mix(count: f64, context: f64, lower: f64) -> f64 {
    if context > 0.0 {
        OWN * count / context + (1.0 - OWN) * lower
    } else {
        lower
    }
}

fn symbol(alphabet: &[char], c: char) -> usize {
    1 + alphabet
        .iter()
        .position(|&a| a == c)
        .expect("words are filtered by the alphabet")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes the table the way the app will: the average cost per symbol of `word`.
    fn cost(table: &[u8], word: &str) -> f64 {
        assert_eq!(&table[..4], MAGIC);
        let n = u16::from_le_bytes([table[6], table[7]]) as usize;
        let len = u32::from_le_bytes(table[8..12].try_into().unwrap()) as usize;
        let alphabet: Vec<char> = std::str::from_utf8(&table[12..12 + len])
            .unwrap()
            .chars()
            .collect();
        let costs = &table[12 + len..];
        assert_eq!(costs.len(), n * n * n);
        let mut seq = vec![0, 0];
        seq.extend(word.chars().map(|c| symbol(&alphabet, c)));
        seq.push(0);
        let sum: u32 = seq
            .windows(3)
            .map(|w| costs[(w[0] * n + w[1]) * n + w[2]] as u32)
            .sum();
        sum as f64 / (seq.len() - 2) as f64
    }

    #[test]
    fn familiar_letter_runs_cost_less() {
        let alphabet: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
        let words = [
            "the", "there", "then", "this", "that", "cat", "hat", "other", "mother", "path",
        ];
        let zipf = words.iter().map(|&w| (w, 50)).collect();
        let table = build(&alphabet, &zipf);
        assert!(cost(&table, "thet") < cost(&table, "tqxz"));
        // Unseen as a word, but made of familiar runs.
        assert!(cost(&table, "mothe") < cost(&table, "hmtoe"));
        assert!(table[12 + alphabet.len()..].iter().all(|&c| c > 0));
    }
}
