//! Language packs for auto-switching (`kbg_core::lng`): of the `.lng` files, only the languages
//! of installed layouts are unpacked into `packs\<code>\` in the data folder (again when the
//! version changes), and their files are mapped, not read. Folders of packs that are gone are
//! removed.

use std::path::Path;
use std::sync::Arc;

use kbg_core::lng::{self, Archive};
use kbg_core::{Detector, LayoutId, Manifest, Pack};

/// `packs\<code>\` with the archive's version, unpacked now if needed. The old files must not be
/// mapped any more.
fn unpacked(archive: &Archive, data: &Path) -> Result<std::path::PathBuf, String> {
    let code = &archive.manifest.code;
    let dir = data.join(lng::FOLDER).join(code);
    let current = std::fs::read_to_string(dir.join(lng::MANIFEST))
        .ok()
        .and_then(|text| Manifest::parse(&text).ok());
    if current.is_none_or(|m| m.version != archive.manifest.version) {
        lng::unpack(archive, &dir)?;
        log::info!("language pack {code} {} unpacked", archive.manifest.version);
    }
    Ok(dir)
}

fn load(dir: &Path) -> Result<Pack, String> {
    let manifest = std::fs::read_to_string(dir.join(lng::MANIFEST)).map_err(|e| e.to_string())?;
    let words = kbg_platform::map_file(&dir.join("words.fst")).map_err(|e| e.to_string())?;
    let trigrams = kbg_platform::map_file(&dir.join("trigrams.bin")).map_err(|e| e.to_string())?;
    Pack::new(&manifest, words, trigrams).map_err(|e| e.to_string())
}

/// Unpacked folders whose `.lng` is gone (a deleted download).
fn remove_orphans(data: &Path, archives: &[Archive]) {
    let Ok(entries) = std::fs::read_dir(data.join(lng::FOLDER)) else {
        return;
    };
    for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
        if !archives
            .iter()
            .any(|a| Some(&a.manifest.code) == name.as_ref())
        {
            match lng::remove_dir(&dir) {
                Ok(()) => log::info!("removed {}", dir.display()),
                Err(e) => log::warn!("{}: {e}", dir.display()),
            }
        }
    }
}

/// A detector for the `installed` layouts that have a pack; `None` when fewer than two do.
/// Nothing may have the unpacked packs mapped.
pub fn detector(installed: &[LayoutId]) -> Option<Detector> {
    let data = crate::data_dir()?;
    let archives = lng::archives(&lng::roots(Some(data)), |path, e| {
        log::warn!("language pack {}: {e}", path.display())
    });
    remove_orphans(data, &archives);
    let mut served: Vec<(LayoutId, Arc<Pack>)> = Vec::new();
    let mut loaded = Vec::new();
    for archive in &archives {
        let layouts: Vec<LayoutId> = installed
            .iter()
            .copied()
            .filter(|&l| kbg_platform::layout_serves(l, &archive.manifest.layouts))
            .collect();
        if layouts.is_empty() {
            continue;
        }
        let pack = match unpacked(archive, data).and_then(|dir| load(&dir)) {
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
