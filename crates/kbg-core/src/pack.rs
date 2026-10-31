//! A language pack as `kbg-pack-builder` writes it: `pack.toml`, `words.fst` (word forms with
//! their frequency) and `trigrams.bin` (the letter model; its format is described in the
//! builder's trigrams.rs). The data comes in as bytes, so the platform can hand over mapped files
//! and the OS pages in only what a lookup touches.

use std::fmt;

use serde::Deserialize;

/// The newest `pack.toml` format this version reads.
pub const FORMAT: u16 = 1;
const MAGIC: &[u8; 4] = b"KBGT";
const TRIGRAM_FORMAT: u16 = 1;
/// Cost units per bit in `trigrams.bin`.
const SCALE: f32 = 8.0;

/// Pack data from anywhere: a mapped file, a `Vec<u8>`...
pub struct Bytes(Box<dyn AsRef<[u8]> + Send + Sync>);

impl Bytes {
    pub fn new(data: impl AsRef<[u8]> + Send + Sync + 'static) -> Self {
        Self(Box::new(data))
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        (*self.0).as_ref()
    }
}

/// `pack.toml`.
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub format: u16,
    pub code: String,
    pub name: String,
    pub version: u32,
    /// Windows keyboard layout ids (KLIDs) the pack serves.
    pub layouts: Vec<String>,
    pub license: String,
    pub attribution: String,
}

impl Manifest {
    /// `pack.toml`'s text; a pack made for a newer app is an error.
    pub fn parse(text: &str) -> Result<Self, PackError> {
        let manifest: Manifest =
            toml::from_str(text).map_err(|e| PackError::Manifest(e.to_string()))?;
        if manifest.format > FORMAT {
            return Err(PackError::Format(manifest.format));
        }
        Ok(manifest)
    }
}

#[derive(Debug)]
pub enum PackError {
    Manifest(String),
    /// Made for a newer app.
    Format(u16),
    Words(String),
    Trigrams,
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PackError::Manifest(e) => write!(f, "pack.toml: {e}"),
            PackError::Format(v) => write!(f, "pack format {v} is newer than {FORMAT}"),
            PackError::Words(e) => write!(f, "words.fst: {e}"),
            PackError::Trigrams => write!(f, "trigrams.bin is damaged"),
        }
    }
}

impl std::error::Error for PackError {}

pub struct Pack {
    pub manifest: Manifest,
    words: fst::Map<Bytes>,
    trigrams: Bytes,
    alphabet: Vec<char>,
    /// Symbols: the letters + the word boundary.
    n: usize,
    /// Where the costs start in `trigrams`.
    costs: usize,
}

impl Pack {
    pub fn new(manifest: &str, words: Bytes, trigrams: Bytes) -> Result<Self, PackError> {
        let manifest = Manifest::parse(manifest)?;
        let words = fst::Map::new(words).map_err(|e| PackError::Words(e.to_string()))?;
        let (alphabet, costs) = trigram_header(trigrams.as_ref()).ok_or(PackError::Trigrams)?;
        let n = alphabet.len() + 1;
        if trigrams.as_ref().len() != costs + n * n * n {
            return Err(PackError::Trigrams);
        }
        Ok(Self {
            manifest,
            words,
            trigrams,
            alphabet,
            n,
            costs,
        })
    }

    /// `word`'s frequency on the Zipf scale x 10 (0 = in the dictionary, frequency unknown);
    /// `None` when it is not a word of this language. `word` must be lowercase.
    pub fn frequency(&self, word: &str) -> Option<u64> {
        self.words.get(word)
    }

    /// How unlike this language `word` looks, in bits per letter (the end of the word counts as
    /// one more): low for familiar letter runs, high for odd ones. `None` when a letter is not in
    /// the alphabet. `word` must be lowercase.
    pub fn letter_cost(&self, word: &str) -> Option<f32> {
        let n = self.n;
        let costs = &self.trigrams.as_ref()[self.costs..];
        let (mut a, mut b) = (0, 0);
        let mut sum = 0u32;
        let mut symbols = 0u32;
        for c in word.chars().map(Some).chain([None]) {
            let s = match c {
                Some(c) => 1 + self.alphabet.iter().position(|&l| l == c)?,
                None => 0,
            };
            sum += costs[(a * n + b) * n + s] as u32;
            symbols += 1;
            (a, b) = (b, s);
        }
        Some(sum as f32 / SCALE / symbols as f32)
    }

    pub fn has_letter(&self, c: char) -> bool {
        self.alphabet.contains(&c)
    }
}

/// The alphabet and where the costs start.
fn trigram_header(data: &[u8]) -> Option<(Vec<char>, usize)> {
    if data.get(..4)? != MAGIC
        || u16::from_le_bytes(data.get(4..6)?.try_into().ok()?) > TRIGRAM_FORMAT
    {
        return None;
    }
    let n = u16::from_le_bytes(data.get(6..8)?.try_into().ok()?) as usize;
    let len = u32::from_le_bytes(data.get(8..12)?.try_into().ok()?) as usize;
    let alphabet: Vec<char> = std::str::from_utf8(data.get(12..12 + len)?)
        .ok()?
        .chars()
        .collect();
    (alphabet.len() + 1 == n).then_some((alphabet, 12 + len))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use kbg_pack_builder::{trigrams, words};

    /// A pack built by the real builder from `words` (`word count` lines).
    pub fn pack(code: &str, alphabet: &str, words: &str) -> Pack {
        let alphabet: Vec<char> = alphabet.chars().collect();
        let mut counts = words::Counts::new();
        words::parse(words, &alphabet, &mut counts);
        let zipf = words::zipf(&counts);
        let manifest = format!(
            "format = 1\ncode = \"{code}\"\nname = \"{code}\"\nversion = 1\nlayouts = []\nlicense = \"\"\nattribution = \"\"\n"
        );
        Pack::new(
            &manifest,
            Bytes::new(words::fst(&zipf)),
            Bytes::new(trigrams::build(&alphabet, &zipf)),
        )
        .unwrap()
    }

    pub const EN_LETTERS: &str = "abcdefghijklmnopqrstuvwxyz'";
    pub const RU_LETTERS: &str = "абвгдеёжзийклмнопрстуфхцчшщъыьэюя";

    #[test]
    fn reads_what_the_builder_writes() {
        let en = pack(
            "en",
            EN_LETTERS,
            "the 900\nhello 50\nthere 40\nworld 30\nother 20\n",
        );
        assert_eq!(en.manifest.code, "en");
        assert!(en.frequency("hello").unwrap() > 0);
        assert_eq!(en.frequency("hellp"), None);
        assert!(en.letter_cost("thet").unwrap() < en.letter_cost("qxzv").unwrap());
        assert_eq!(en.letter_cost("thé"), None);
        assert!(en.has_letter('\'') && !en.has_letter('ж'));
    }

    #[test]
    fn rejects_damaged_and_newer_packs() {
        let manifest = "format = 2\ncode = \"x\"\nname = \"x\"\nversion = 1\nlayouts = []\nlicense = \"\"\nattribution = \"\"\n";
        let empty = || Bytes::new(Vec::new());
        assert!(matches!(
            Pack::new(manifest, empty(), empty()),
            Err(PackError::Format(2))
        ));
        let manifest = manifest.replace("format = 2", "format = 1");
        assert!(matches!(
            Pack::new(&manifest, empty(), empty()),
            Err(PackError::Words(_))
        ));
        let words = Bytes::new(
            fst::Map::from_iter([("a", 1)])
                .unwrap()
                .into_fst()
                .into_inner(),
        );
        let trigrams = Bytes::new(b"KBGT\x01\x00\x02\x00\x01\x00\x00\x00a".to_vec());
        assert!(matches!(
            Pack::new(&manifest, words, trigrams),
            Err(PackError::Trigrams)
        ));
    }
}
