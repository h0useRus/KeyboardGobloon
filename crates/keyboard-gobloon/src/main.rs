// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logfile;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, sync_channel};

use gob_i18n::I18n;
use kbg_core::{Command, Engine, Input};
use kbg_platform::{SystemLayouts, TrayMenu};

pub const APP_NAME: &str = "Keyboard Gobloon";
/// Set by build.rs: CI's release version, else git describe, else the crate version.
pub const VERSION: &str = env!("KEYBOARD_GOBLOON_VERSION");

pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", include_str!("../../../lang/en.ftl")),
    ("ru", include_str!("../../../lang/ru.ftl")),
];

/// Room for bursts of keys while the engine thread is busy typing a conversion.
const INPUT_QUEUE: usize = 256;

/// Folder of settings and the log: the exe's folder when it holds a `settings.gob` (portable
/// copy), else the user's application data.
pub fn data_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let local = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .filter(|dir| dir.join("settings.gob").is_file());
        local.or_else(kbg_platform::default_data_dir)
    })
    .as_deref()
}

fn i18n() -> I18n {
    let lang_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("lang")));
    let mut i18n = I18n::new(LANGUAGES, lang_dir.as_deref());
    if let Some(code) = kbg_platform::user_language() {
        i18n.set_language(&code);
    }
    i18n
}

/// Runs the engine on the hooks' input until the hooks stop.
fn run_engine(rx: Receiver<Input>) {
    let layouts = SystemLayouts;
    let mut engine = Engine::new();
    for input in rx {
        for command in engine.handle(input, &layouts) {
            match command {
                Command::Replace { erase, text } => kbg_platform::replace(erase, &text),
                Command::SwitchLayout(layout) => kbg_platform::switch_layout(layout),
            }
        }
    }
}

fn main() {
    if !kbg_platform::claim_instance() {
        return;
    }
    logfile::init();
    let i18n = i18n();
    let (tx, rx) = sync_channel(INPUT_QUEUE);
    let Some(hook) = kbg_platform::start_input(tx) else {
        log::error!("no keyboard input: exiting");
        return;
    };
    let engine = std::thread::Builder::new()
        .name("kbg-engine".into())
        .spawn(move || run_engine(rx));
    kbg_platform::run_tray(&TrayMenu {
        tooltip: APP_NAME.into(),
        exit: i18n.tr("tray-exit"),
    });
    // Removing the hooks closes the channel, which ends the engine thread.
    drop(hook);
    if let Ok(engine) = engine {
        let _ = engine.join();
    }
    log::info!("exited");
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_language_has_every_key() {
        assert_eq!(gob_i18n::check_complete(super::LANGUAGES), Ok(()));
    }

    #[test]
    fn languages_load() {
        let mut i18n = gob_i18n::I18n::new(super::LANGUAGES, None);
        i18n.set_language("ru");
        assert_eq!(i18n.tr("tray-exit"), "Выход");
        assert_eq!(i18n.languages().len(), super::LANGUAGES.len());
    }
}
