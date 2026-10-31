//! Language packs for auto-switching. A pack ships as `<code>.lng` (a zip, see kbg-pack-builder)
//! in `packs\` next to the exe (bundled) or in the data folder (downloaded); of two copies of a
//! language the newer version wins. Only the languages of installed layouts are unpacked, into
//! `packs\<code>\` in the data folder, and their files are mapped, not read.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kbg_core::{Detector, LayoutId, Manifest, Pack};

const FOLDER: &str = "packs";
const EXTENSION: &str = "lng";
const MANIFEST: &str = "pack.toml";

struct Archive {
    path: PathBuf,
    manifest: Manifest,
}

fn roots() -> Vec<PathBuf> {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(FOLDER)));
    let mut roots: Vec<PathBuf> = crate::data_dir()
        .map(|dir| dir.join(FOLDER))
        .into_iter()
        .chain(bundled)
        .collect();
    // A portable copy keeps its data next to the exe.
    roots.dedup();
    roots
}

fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut text = String::new();
    zip.by_name(MANIFEST)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    Manifest::parse(&text).map_err(|e| e.to_string())
}

/// Every `.lng`, the newest of each language.
fn archives() -> Vec<Archive> {
    let mut archives: Vec<Archive> = Vec::new();
    for root in roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if path.extension().is_none_or(|e| e != EXTENSION) {
                continue;
            }
            let manifest = match read_manifest(&path) {
                Ok(manifest) => manifest,
                Err(e) => {
                    log::warn!("language pack {}: {e}", path.display());
                    continue;
                }
            };
            let pack = Archive { path, manifest };
            match archives
                .iter_mut()
                .find(|a| a.manifest.code == pack.manifest.code)
            {
                Some(old) if old.manifest.version < pack.manifest.version => *old = pack,
                Some(_) => {}
                None => archives.push(pack),
            }
        }
    }
    archives
}

/// `packs\<code>\` in the data folder with the archive's files, unpacked now unless that version
/// is there already. The old files must not be mapped any more.
fn unpacked(archive: &Archive, data: &Path) -> Result<PathBuf, String> {
    let code = &archive.manifest.code;
    let dir = data.join(FOLDER).join(code);
    let current = std::fs::read_to_string(dir.join(MANIFEST))
        .ok()
        .and_then(|text| Manifest::parse(&text).ok());
    if current.is_some_and(|m| m.version == archive.manifest.version) {
        return Ok(dir);
    }
    // Unpacked next to it and renamed, so a broken unpack never leaves half a pack.
    let new = data.join(FOLDER).join(format!("{code}.new"));
    let _ = std::fs::remove_dir_all(&new);
    std::fs::create_dir_all(&new).map_err(|e| e.to_string())?;
    let file = File::open(&archive.path).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(file)
        .and_then(|mut zip| zip.extract(&new))
        .map_err(|e| e.to_string())?;
    match std::fs::remove_dir_all(&dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    std::fs::rename(&new, &dir).map_err(|e| e.to_string())?;
    log::info!("language pack {code} {} unpacked", archive.manifest.version);
    Ok(dir)
}

fn load(dir: &Path) -> Result<Pack, String> {
    let manifest = std::fs::read_to_string(dir.join(MANIFEST)).map_err(|e| e.to_string())?;
    let words = kbg_platform::map_file(&dir.join("words.fst")).map_err(|e| e.to_string())?;
    let trigrams = kbg_platform::map_file(&dir.join("trigrams.bin")).map_err(|e| e.to_string())?;
    Pack::new(&manifest, words, trigrams).map_err(|e| e.to_string())
}

/// A detector for the `installed` layouts that have a pack; `None` when fewer than two do.
pub fn detector(installed: &[LayoutId]) -> Option<Detector> {
    let data = crate::data_dir()?;
    let mut served: Vec<(LayoutId, Arc<Pack>)> = Vec::new();
    let mut loaded = Vec::new();
    for archive in archives() {
        let layouts: Vec<LayoutId> = installed
            .iter()
            .copied()
            .filter(|&l| kbg_platform::layout_serves(l, &archive.manifest.layouts))
            .collect();
        if layouts.is_empty() {
            continue;
        }
        let pack = match unpacked(&archive, data).and_then(|dir| load(&dir)) {
            Ok(pack) => Arc::new(pack),
            Err(e) => {
                log::warn!("language pack {}: {e}", archive.path.display());
                continue;
            }
        };
        loaded.push(format!("{} {}", pack.manifest.code, pack.manifest.version));
        served.extend(layouts.into_iter().map(|l| (l, Arc::clone(&pack))));
    }
    log::info!(
        "language packs: {} for {} of {} layouts",
        loaded.join(", "),
        served.len(),
        installed.len()
    );
    (served.len() >= 2).then(|| Detector::new(served))
}
