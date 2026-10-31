//! The Languages tab: the language packs auto-switching uses. English and Russian come with the
//! app (`packs\` next to it); the others download from the catalog of the GitHub release
//! `packs` into `packs\` in the data folder. Network use starts only when this tab opens.
//! Downloads run on a thread and must match the catalog's size and SHA-256; then the running app
//! is told to load the packs again (it unpacks only languages with a Windows layout).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{self, RichText};
use gob_i18n::I18n;
use gob_ui::icon;
use kbg_core::Layouts;
use kbg_core::lng::{self, Archive};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const CATALOG: &str =
    "https://github.com/h0useRus/KeyboardGobloon/releases/download/packs/packs.json";
/// Larger answers are not packs.
const MAX_SIZE: u64 = 64 << 20;

#[derive(Clone, Deserialize)]
struct Entry {
    code: String,
    name: String,
    version: u32,
    size: u64,
    sha256: String,
    url: String,
}

#[derive(Deserialize)]
struct Catalog {
    packs: Vec<Entry>,
}

enum Done {
    Catalog(Result<Vec<Entry>, String>),
    Download {
        code: String,
        name: String,
        result: Result<(), String>,
    },
}

pub struct Languages {
    data: Option<PathBuf>,
    /// `None` until the tab first opens; then loading until the thread answers.
    catalog: Option<Option<Result<Vec<Entry>, String>>>,
    /// Codes being downloaded.
    busy: Vec<String>,
    archives: Vec<Archive>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

fn agent() -> ureq::Agent {
    // Windows' own TLS and certificate store.
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build()
        .into()
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent().get(url).call().map_err(|e| e.to_string())?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_SIZE)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

fn fetch_catalog() -> Result<Vec<Entry>, String> {
    let catalog: Catalog = serde_json::from_slice(&get(CATALOG)?).map_err(|e| e.to_string())?;
    Ok(catalog.packs)
}

/// Downloads `entry` to `<dir>\<code>.lng`: whole and checked, or not at all.
fn download(entry: &Entry, dir: &Path) -> Result<(), String> {
    let bytes = get(&entry.url)?;
    let sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if bytes.len() as u64 != entry.size || sha256 != entry.sha256 {
        return Err("the file is damaged".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.{}", entry.code, lng::EXTENSION));
    let part = path.with_extension("part");
    std::fs::write(&part, &bytes).map_err(|e| e.to_string())?;
    // A pack that is not one this app reads never takes the place of a working one.
    lng::manifest(&part).inspect_err(|_| {
        let _ = std::fs::remove_file(&part);
    })?;
    std::fs::rename(&part, &path).map_err(|e| e.to_string())
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1 << 20) as f64)
}

impl Languages {
    pub fn new(data: Option<PathBuf>) -> Self {
        let (tx, rx) = channel();
        let mut languages = Self {
            data,
            catalog: None,
            busy: Vec::new(),
            archives: Vec::new(),
            tx,
            rx,
        };
        languages.rescan();
        languages
    }

    fn rescan(&mut self) {
        self.archives = lng::archives(&lng::roots(self.data.as_deref()), |_, _| {});
    }

    fn downloads(&self) -> Option<PathBuf> {
        self.data.as_ref().map(|d| d.join(lng::FOLDER))
    }

    fn load_catalog(&mut self, ctx: &egui::Context) {
        self.catalog = Some(None);
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Done::Catalog(fetch_catalog()));
            ctx.request_repaint();
        });
    }

    fn start_download(&mut self, entry: Entry, ctx: &egui::Context) {
        let Some(dir) = self.downloads() else { return };
        self.busy.push(entry.code.clone());
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = download(&entry, &dir);
            let _ = tx.send(Done::Download {
                code: entry.code,
                name: entry.name,
                result,
            });
            ctx.request_repaint();
        });
    }

    /// Answers from the threads; a status line note when something finished.
    fn poll(&mut self, i18n: &I18n) -> Option<String> {
        let mut note = None;
        while let Ok(done) = self.rx.try_recv() {
            match done {
                Done::Catalog(result) => self.catalog = Some(Some(result)),
                Done::Download { code, name, result } => {
                    self.busy.retain(|c| *c != code);
                    self.rescan();
                    note = Some(match result {
                        Ok(()) => {
                            kbg_platform::notify_app();
                            i18n.trf("status-pack-downloaded", &[("name", name.into())])
                        }
                        Err(e) => i18n.trf(
                            "status-pack-failed",
                            &[("name", name.into()), ("error", e.into())],
                        ),
                    });
                }
            }
        }
        note
    }

    fn catalog_entries(&self) -> &[Entry] {
        match &self.catalog {
            Some(Some(Ok(entries))) => entries,
            _ => &[],
        }
    }

    /// Draws the tab; returns a note for the status line.
    pub fn show(&mut self, ui: &mut egui::Ui, i18n: &I18n) -> Option<String> {
        if self.catalog.is_none() {
            self.load_catalog(ui.ctx());
        }
        let mut note = self.poll(i18n);
        ui.label(RichText::new(i18n.tr("languages-intro")).weak());
        ui.add_space(gob_ui::SPACE_M);

        let installed_layouts = kbg_platform::SystemLayouts.installed();
        let downloads = self.downloads();
        // Every language we know of: installed ones first, then the catalog's others.
        let mut codes: Vec<String> = self
            .archives
            .iter()
            .map(|a| a.manifest.code.clone())
            .collect();
        for entry in self.catalog_entries() {
            if !codes.contains(&entry.code) {
                codes.push(entry.code.clone());
            }
        }
        let mut download = None;
        let mut delete = None;
        egui::Grid::new("languages")
            .num_columns(3)
            .spacing([gob_ui::SPACE_L, gob_ui::SPACE_S])
            .show(ui, |ui| {
                for code in &codes {
                    let archive = self.archives.iter().find(|a| &a.manifest.code == code);
                    let entry = self.catalog_entries().iter().find(|e| &e.code == code);
                    let (name, layouts) = match (archive, entry) {
                        (Some(a), _) => (a.manifest.name.clone(), a.manifest.layouts.clone()),
                        (None, Some(e)) => (e.name.clone(), Vec::new()),
                        (None, None) => continue,
                    };
                    ui.label(RichText::new(format!("{}  {name}", icon::TRANSLATE)).strong());

                    let downloaded = archive
                        .is_some_and(|a| downloads.as_ref().is_some_and(|d| a.path.starts_with(d)));
                    let newer =
                        entry.filter(|e| archive.is_none_or(|a| e.version > a.manifest.version));
                    ui.horizontal(|ui| {
                        match archive {
                            Some(a) if downloaded => ui.label(i18n.trf(
                                "languages-installed",
                                &[("version", a.manifest.version.into())],
                            )),
                            Some(_) => ui.label(i18n.tr("languages-included")),
                            None => ui.label(
                                RichText::new(megabytes(entry.map_or(0, |e| e.size))).weak(),
                            ),
                        };
                        let unused = archive.is_some()
                            && !installed_layouts
                                .iter()
                                .any(|&l| kbg_platform::layout_serves(l, &layouts));
                        if unused {
                            ui.label(
                                RichText::new(icon::WARNING).color(ui.visuals().warn_fg_color),
                            )
                            .on_hover_text(
                                i18n.trf("languages-no-layout", &[("name", name.clone().into())]),
                            );
                        }
                    });

                    ui.horizontal(|ui| {
                        if self.busy.contains(code) {
                            ui.spinner();
                        } else if let Some(e) = newer {
                            let (glyph, tip) = if archive.is_some() {
                                (icon::ARROWS_CLOCKWISE, "languages-update")
                            } else {
                                (icon::DOWNLOAD_SIMPLE, "languages-download")
                            };
                            if gob_ui::icon_button(ui, downloads.is_some(), glyph, &i18n.tr(tip))
                                .clicked()
                            {
                                download = Some(e.clone());
                            }
                        }
                        if downloaded
                            && !self.busy.contains(code)
                            && gob_ui::icon_button(
                                ui,
                                true,
                                icon::TRASH,
                                &i18n.tr("languages-delete"),
                            )
                            .clicked()
                        {
                            delete = archive.map(|a| (a.path.clone(), name.clone()));
                        }
                    });
                    ui.end_row();
                }
            });

        match &self.catalog {
            Some(None) => {
                ui.add_space(gob_ui::SPACE_S);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(i18n.tr("languages-loading")).weak());
                });
            }
            Some(Some(Err(e))) => {
                ui.add_space(gob_ui::SPACE_S);
                let error = e.clone();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(i18n.trf("languages-offline", &[("error", error.into())]))
                            .color(ui.visuals().warn_fg_color),
                    );
                    if gob_ui::icon_button(
                        ui,
                        true,
                        icon::ARROWS_CLOCKWISE,
                        &i18n.tr("languages-retry"),
                    )
                    .clicked()
                    {
                        self.load_catalog(ui.ctx());
                    }
                });
            }
            _ => {}
        }

        if let Some(entry) = download {
            self.start_download(entry, ui.ctx());
        }
        if let Some((path, name)) = delete {
            note = Some(match std::fs::remove_file(&path) {
                Ok(()) => {
                    // The app removes the unpacked folder when it loads the packs again.
                    kbg_platform::notify_app();
                    i18n.trf("status-pack-deleted", &[("name", name.into())])
                }
                Err(e) => e.to_string(),
            });
            self.rescan();
        }
        note
    }
}
