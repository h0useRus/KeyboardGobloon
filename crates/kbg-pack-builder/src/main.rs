//! `kbg-pack-builder [--url <base>] --out <dir> <source dir>...`
//! Builds each source folder into `<dir>/<code>-<version>.zip` and writes `<dir>/packs.json`.
//! `--url` is where the zips will be downloaded from (a release's download folder); without it
//! the catalog holds bare file names.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kbg-pack-builder: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut out = None;
    let mut url_base = String::new();
    let mut sources = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--url" => url_base = args.next().ok_or("--url needs a value")?,
            _ if arg.starts_with("--") => return Err(format!("unknown option {arg}")),
            _ => sources.push(PathBuf::from(arg)),
        }
    }
    let out = out.ok_or("usage: kbg-pack-builder [--url <base>] --out <dir> <source dir>...")?;
    if sources.is_empty() {
        return Err("no source folders".into());
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    let mut catalog = kbg_pack_builder::Catalog::default();
    for dir in &sources {
        let pack = kbg_pack_builder::build(dir)?;
        let zip = pack.zip();
        let path = out.join(pack.file_name());
        std::fs::write(&path, &zip).map_err(|e| format!("{}: {e}", path.display()))?;
        println!(
            "{}: {} words, {} lines skipped, {} KB",
            path.display(),
            pack.words,
            pack.skipped,
            zip.len() / 1024
        );
        catalog.packs.push(pack.catalog_entry(&zip, &url_base));
    }
    let path = out.join("packs.json");
    let json = serde_json::to_string_pretty(&catalog).expect("plain values");
    std::fs::write(&path, json).map_err(|e| format!("{}: {e}", path.display()))
}
