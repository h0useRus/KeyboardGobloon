//! Builds Keyboard Gobloon's language packs. A source folder holds `source.toml` (see `Source`),
//! the word lists and the data's license files; the pack is one `<code>.lng` file, a zip with
//! - `pack.toml`: code, name, version, the Windows layouts (KLIDs) it serves, license,
//! - `words.fst`: every word form with its frequency (`words`),
//! - `trigrams.bin`: the letter model (`trigrams`),
//! - the license files.
//!
//! The app unpacks a `.lng` only when an installed layout needs its language. `packs.json` lists
//! the packs with size and SHA-256; the app downloads packs from it.

pub mod trigrams;
pub mod words;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SOURCE: &str = "source.toml";
/// The pack file's extension: `en.lng`.
pub const EXTENSION: &str = "lng";
/// The pack layout version in `pack.toml`; the app ignores packs newer than it knows.
pub const FORMAT: u16 = 1;

/// `source.toml`. Paths are relative to its folder.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Language code, also the pack's folder name: `en`, `ru`.
    pub code: String,
    /// The language's own name: `English`, `Русский`.
    pub name: String,
    /// Raised whenever the data changes, so the app offers the update.
    pub version: u32,
    /// Windows keyboard layout ids (KLIDs, 8 hex digits) this pack serves.
    pub layouts: Vec<String>,
    /// SPDX id of the data's license.
    pub license: String,
    /// Where the data comes from, shown on the About tab.
    pub attribution: String,
    /// Every letter a word may contain, lowercase. Words with other characters are skipped.
    pub alphabet: String,
    /// Word lists: `word` or `word count` per line.
    pub words: Vec<PathBuf>,
    /// Copied into the pack as they are.
    #[serde(default)]
    pub license_files: Vec<PathBuf>,
}

#[derive(Serialize)]
struct Manifest<'a> {
    format: u16,
    code: &'a str,
    name: &'a str,
    version: u32,
    layouts: &'a [String],
    license: &'a str,
    attribution: &'a str,
}

pub struct Pack {
    pub source: Source,
    /// Zip entries in order.
    pub files: Vec<(String, Vec<u8>)>,
    pub words: usize,
    /// Word-list lines that were not used (foreign letters, broken lines).
    pub skipped: usize,
}

#[derive(Debug, Serialize)]
pub struct CatalogEntry {
    pub code: String,
    pub name: String,
    pub version: u32,
    pub size: u64,
    pub sha256: String,
    pub url: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Catalog {
    pub packs: Vec<CatalogEntry>,
}

pub fn build(dir: &Path) -> Result<Pack, String> {
    let source: Source = toml::from_str(&read_text(&dir.join(SOURCE))?)
        .map_err(|e| format!("{}: {e}", dir.join(SOURCE).display()))?;
    check(&source)?;
    let alphabet: Vec<char> = source.alphabet.chars().collect();
    let mut counts = words::Counts::new();
    let mut skipped = 0;
    for file in &source.words {
        skipped += words::parse(&read_text(&dir.join(file))?, &alphabet, &mut counts);
    }
    if counts.is_empty() {
        return Err(format!("{}: no words", source.code));
    }
    let zipf = words::zipf(&counts);
    let manifest = Manifest {
        format: FORMAT,
        code: &source.code,
        name: &source.name,
        version: source.version,
        layouts: &source.layouts,
        license: &source.license,
        attribution: &source.attribution,
    };
    let mut files = vec![
        (
            "pack.toml".to_string(),
            toml::to_string(&manifest)
                .expect("plain values")
                .into_bytes(),
        ),
        ("words.fst".to_string(), words::fst(&zipf)),
        (
            "trigrams.bin".to_string(),
            trigrams::build(&alphabet, &zipf),
        ),
    ];
    for file in &source.license_files {
        let name = file
            .file_name()
            .ok_or_else(|| format!("{}: not a file", file.display()))?;
        let bytes =
            std::fs::read(dir.join(file)).map_err(|e| format!("{}: {e}", file.display()))?;
        files.push((name.to_string_lossy().into_owned(), bytes));
    }
    Ok(Pack {
        words: counts.len(),
        skipped,
        source,
        files,
    })
}

fn check(source: &Source) -> Result<(), String> {
    let code = &source.code;
    if !(2..=3).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err(format!("code `{code}`: 2 or 3 lowercase letters"));
    }
    if source.layouts.is_empty() {
        return Err(format!("{code}: no layouts"));
    }
    if let Some(klid) = source
        .layouts
        .iter()
        .find(|k| k.len() != 8 || !k.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(format!(
            "{code}: layout `{klid}` is not a KLID (8 hex digits)"
        ));
    }
    let alphabet: Vec<char> = source.alphabet.chars().collect();
    for (i, c) in alphabet.iter().enumerate() {
        if alphabet[..i].contains(c) || c.is_whitespace() || c.to_lowercase().ne([*c]) {
            return Err(format!(
                "{code}: alphabet letter `{c}` is repeated, a space or uppercase"
            ));
        }
    }
    if alphabet.is_empty() || source.words.is_empty() {
        return Err(format!("{code}: empty alphabet or no word lists"));
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

impl Pack {
    /// `en.lng` (the version is in the catalog and in `pack.toml`).
    pub fn file_name(&self) -> String {
        format!("{}.{EXTENSION}", self.source.code)
    }

    /// The zip. Entries carry a fixed date, so the same data gives the same bytes and hash.
    pub fn zip(&self) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (name, bytes) in &self.files {
            zip.start_file(name.as_str(), options)
                .expect("writing to memory");
            zip.write_all(bytes).expect("writing to memory");
        }
        zip.finish().expect("writing to memory").into_inner()
    }

    /// The catalog entry for `zip` (the bytes of `zip()`), downloaded from `url_base/file_name()`.
    pub fn catalog_entry(&self, zip: &[u8], url_base: &str) -> CatalogEntry {
        let sha256 = Sha256::digest(zip)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let url = if url_base.is_empty() {
            self.file_name()
        } else {
            format!("{}/{}", url_base.trim_end_matches('/'), self.file_name())
        };
        CatalogEntry {
            code: self.source.code.clone(),
            name: self.source.name.clone(),
            version: self.source.version,
            size: zip.len() as u64,
            sha256,
            url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_dir(name: &str, toml: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kbg-pack-builder-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SOURCE), toml).unwrap();
        std::fs::write(dir.join("words.txt"), "кот 40\nКит 10\nкто\nzoo 3\n").unwrap();
        std::fs::write(dir.join("LICENSE.txt"), "data license").unwrap();
        dir
    }

    const RU: &str = r#"
code = "ru"
name = "Русский"
version = 2
layouts = ["00000419"]
license = "CC-BY-SA-4.0"
attribution = "test words"
alphabet = "абвгдеёжзийклмнопрстуфхцчшщъыьэюя"
words = ["words.txt"]
license_files = ["LICENSE.txt"]
"#;

    #[test]
    fn builds_a_pack_and_its_catalog_entry() {
        let dir = source_dir("ok", RU);
        let pack = build(&dir).unwrap();
        assert_eq!((pack.words, pack.skipped), (3, 1));
        let names: Vec<&str> = pack.files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["pack.toml", "words.fst", "trigrams.bin", "LICENSE.txt"]
        );
        let manifest = String::from_utf8(pack.files[0].1.clone()).unwrap();
        assert!(manifest.contains("format = 1") && manifest.contains("layouts = [\"00000419\"]"));
        let words = fst::Map::new(pack.files[1].1.clone()).unwrap();
        assert!(words.get("кит").is_some() && words.get("кто") == Some(0));

        let zip = pack.zip();
        assert_eq!(zip, pack.zip());
        let entry = pack.catalog_entry(&zip, "https://example.org/packs/");
        assert_eq!(entry.url, "https://example.org/packs/ru.lng");
        assert_eq!((entry.size, entry.sha256.len()), (zip.len() as u64, 64));
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
        assert_eq!(archive.len(), 4);
        assert_eq!(archive.by_name("LICENSE.txt").unwrap().size(), 12);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_bad_sources() {
        for (name, from, to) in [
            ("code", "code = \"ru\"", "code = \"RU\""),
            ("klid", "\"00000419\"", "\"0419\""),
            ("alphabet", "alphabet = \"а", "alphabet = \"аа"),
            ("field", "version = 2", "version = 2\nextra = 1"),
        ] {
            let dir = source_dir(name, &RU.replace(from, to));
            assert!(build(&dir).is_err(), "{name}");
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
