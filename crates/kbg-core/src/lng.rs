//! `.lng` files: a language pack as one zip, `pack.toml` first (kbg-pack-builder writes them).
//! They live in `packs\` next to the exe (bundled) and in the data folder (downloaded); the app
//! unpacks only the languages it needs, into `packs\<code>\` in the data folder.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::pack::Manifest;

pub const FOLDER: &str = "packs";
pub const EXTENSION: &str = "lng";
pub const MANIFEST: &str = "pack.toml";

/// A `.lng` file and its manifest.
pub struct Archive {
    pub path: PathBuf,
    pub manifest: Manifest,
}

/// `<data>\packs` (downloaded) and `packs` next to the exe (bundled); once when they are one.
pub fn roots(data: Option<&Path>) -> Vec<PathBuf> {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(FOLDER)));
    let mut roots: Vec<PathBuf> = data
        .map(|d| d.join(FOLDER))
        .into_iter()
        .chain(bundled)
        .collect();
    roots.dedup();
    roots
}

/// The manifest inside a `.lng`, without unpacking it.
pub fn manifest(path: &Path) -> Result<Manifest, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut text = String::new();
    zip.by_name(MANIFEST)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    Manifest::parse(&text).map_err(|e| e.to_string())
}

/// Every `.lng` in `roots`, the newest of each language; broken ones go to `broken`.
pub fn archives(roots: &[PathBuf], mut broken: impl FnMut(&Path, &str)) -> Vec<Archive> {
    let mut archives: Vec<Archive> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if path.extension().is_none_or(|e| e != EXTENSION) {
                continue;
            }
            let manifest = match manifest(&path) {
                Ok(manifest) => manifest,
                Err(e) => {
                    broken(&path, &e);
                    continue;
                }
            };
            let archive = Archive { path, manifest };
            match archives
                .iter_mut()
                .find(|a| a.manifest.code == archive.manifest.code)
            {
                Some(old) if old.manifest.version < archive.manifest.version => *old = archive,
                Some(_) => {}
                None => archives.push(archive),
            }
        }
    }
    archives
}

/// Unpacks `archive` into `dir`, replacing what is there. It goes to `<dir>.new` first and is
/// renamed, so a failed unpack never leaves half a pack. Nothing may have `dir`'s files mapped.
pub fn unpack(archive: &Archive, dir: &Path) -> Result<(), String> {
    let new = dir.with_extension("new");
    let _ = std::fs::remove_dir_all(&new);
    std::fs::create_dir_all(&new).map_err(|e| e.to_string())?;
    let file = File::open(&archive.path).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(file)
        .and_then(|mut zip| zip.extract(&new))
        .map_err(|e| e.to_string())?;
    remove_dir(dir)?;
    std::fs::rename(&new, dir).map_err(|e| e.to_string())
}

/// Removes `dir` and everything in it; a missing one is fine.
pub fn remove_dir(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lng(dir: &Path, code: &str, version: u32) -> PathBuf {
        let source = dir.join(format!("src-{code}-{version}"));
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(
            source.join("source.toml"),
            format!(
                "code = \"{code}\"\nname = \"x\"\nversion = {version}\nlayouts = [\"00000409\"]\nlicense = \"\"\nattribution = \"\"\nalphabet = \"abc\"\nwords = [\"w.txt\"]\n"
            ),
        )
        .unwrap();
        std::fs::write(source.join("w.txt"), "abc 5\ncab 2\n").unwrap();
        let pack = kbg_pack_builder::build(&source).unwrap();
        let path = dir.join(format!("v{version}")).join(pack.file_name());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, pack.zip()).unwrap();
        path
    }

    #[test]
    fn finds_the_newest_and_unpacks_it() {
        let dir = std::env::temp_dir().join(format!("kbg-lng-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        lng(&dir, "en", 1);
        lng(&dir, "en", 2);
        std::fs::write(dir.join("v1").join("bad.lng"), "not a zip").unwrap();
        let roots = [dir.join("v1"), dir.join("v2"), dir.join("missing")];
        let mut bad = Vec::new();
        let found = archives(&roots, |path, _| bad.push(path.to_path_buf()));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manifest.version, 2);
        assert_eq!(bad, [dir.join("v1").join("bad.lng")]);

        let target = dir.join("unpacked").join("en");
        unpack(&found[0], &target).unwrap();
        unpack(&found[0], &target).unwrap();
        let words = std::fs::read(target.join("words.fst")).unwrap();
        let trigrams = std::fs::read(target.join("trigrams.bin")).unwrap();
        let manifest = std::fs::read_to_string(target.join(MANIFEST)).unwrap();
        let pack = crate::Pack::new(
            &manifest,
            crate::Bytes::new(words),
            crate::Bytes::new(trigrams),
        );
        assert_eq!(pack.unwrap().manifest.version, 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
