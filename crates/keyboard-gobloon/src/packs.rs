//! Language packs for auto-switching: `packs\<code>\` in the data folder (downloaded) and next to
//! the exe (bundled); of two copies of a language the newer version wins. Each installed layout
//! gets the pack of its language; the files are mapped, not read.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kbg_core::{Detector, LayoutId, Pack};

const FOLDER: &str = "packs";

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

fn load(dir: &Path) -> Option<Pack> {
    let manifest = std::fs::read_to_string(dir.join("pack.toml")).ok()?;
    let pack = kbg_platform::map_file(&dir.join("words.fst"))
        .and_then(|words| Ok((words, kbg_platform::map_file(&dir.join("trigrams.bin"))?)))
        .map_err(|e| e.to_string())
        .and_then(|(words, trigrams)| {
            Pack::new(&manifest, words, trigrams).map_err(|e| e.to_string())
        });
    pack.inspect_err(|e| log::warn!("language pack {}: {e}", dir.display()))
        .ok()
}

/// A detector for the `installed` layouts that have a pack; `None` when fewer than two do.
pub fn detector(installed: &[LayoutId]) -> Option<Detector> {
    let mut packs: Vec<Pack> = Vec::new();
    for root in roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for pack in entries.flatten().filter_map(|e| load(&e.path())) {
            match packs
                .iter_mut()
                .find(|p| p.manifest.code == pack.manifest.code)
            {
                Some(old) if old.manifest.version < pack.manifest.version => *old = pack,
                Some(_) => {}
                None => packs.push(pack),
            }
        }
    }
    let packs: Vec<Arc<Pack>> = packs.into_iter().map(Arc::new).collect();
    let served: Vec<(LayoutId, Arc<Pack>)> = installed
        .iter()
        .filter_map(|&layout| {
            let pack = packs
                .iter()
                .find(|p| kbg_platform::layout_serves(layout, &p.manifest.layouts))?;
            Some((layout, Arc::clone(pack)))
        })
        .collect();
    let codes: Vec<String> = packs
        .iter()
        .map(|p| format!("{} {}", p.manifest.code, p.manifest.version))
        .collect();
    log::info!(
        "language packs: {} for {} of {} layouts",
        codes.join(", "),
        served.len(),
        installed.len()
    );
    (served.len() >= 2).then(|| Detector::new(served))
}
