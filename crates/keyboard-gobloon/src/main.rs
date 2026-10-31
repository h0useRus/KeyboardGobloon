// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logfile;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, sync_channel};

use gob_i18n::I18n;
use kbg_core::{Command, Engine, Input};
use kbg_platform::{SystemLayouts, TrayEvent, TrayMenu};
use kbg_settings::{Hotkeys, Settings};

pub const APP_NAME: &str = "Keyboard Gobloon";
/// Set by build.rs: CI's release version, else git describe, else the crate version.
pub const VERSION: &str = env!("KEYBOARD_GOBLOON_VERSION");

pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", include_str!("../../../lang/en.ftl")),
    ("ru", include_str!("../../../lang/ru.ftl")),
];

/// The settings window next to this exe; its window title is the product name.
const SETTINGS_EXE: &str = "KeyboardGobloonSettings.exe";

/// Room for bursts of keys while the engine thread is busy typing a conversion.
const INPUT_QUEUE: usize = 256;

/// Folder of settings and the log (a portable copy keeps them next to the exe).
pub fn data_dir() -> Option<&'static Path> {
    kbg_settings::data_dir(kbg_platform::default_data_dir)
}

fn load_settings() -> Settings {
    data_dir().map(Settings::load).unwrap_or_default()
}

fn i18n(settings: &Settings) -> I18n {
    let lang_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("lang")));
    let mut i18n = I18n::new(LANGUAGES, lang_dir.as_deref());
    let code = Some(settings.language.clone())
        .filter(|c| !c.is_empty())
        .or_else(kbg_platform::user_language);
    if let Some(code) = code {
        i18n.set_language(&code);
    }
    i18n
}

fn tray_menu(i18n: &I18n) -> TrayMenu {
    TrayMenu {
        tooltip: APP_NAME.into(),
        enabled: i18n.tr("tray-enabled"),
        settings: i18n.tr("tray-settings"),
        exit: i18n.tr("tray-exit"),
    }
}

/// Opens the settings window, or brings it to the front when it is open already.
fn open_settings() {
    if kbg_platform::focus_window(APP_NAME) {
        return;
    }
    let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(SETTINGS_EXE)))
    else {
        return;
    };
    kbg_platform::allow_foreground();
    if let Err(e) = std::process::Command::new(&exe).spawn() {
        log::error!("{} could not be started: {e}", exe.display());
    }
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
                Command::ConvertSelection => {
                    let mut target = None;
                    kbg_platform::transform_selection(|text| {
                        let (converted, layout) = kbg_core::convert_text(text, &layouts)?;
                        target = Some(layout);
                        Some(converted)
                    });
                    // Keep typing in the language the text is in now.
                    if let Some(layout) = target {
                        kbg_platform::switch_layout(layout);
                    }
                }
                Command::ChangeCase => kbg_platform::transform_selection(|text| {
                    let changed = kbg_core::cycle_case(text);
                    (changed != text).then_some(changed)
                }),
            }
        }
        kbg_platform::notify_input();
    }
}

fn main() {
    if !kbg_platform::claim_instance() {
        return;
    }
    logfile::init();
    let settings = load_settings();
    kbg_platform::set_hotkeys(&settings.hotkeys);
    let menu = tray_menu(&i18n(&settings));
    let (tx, rx) = sync_channel(INPUT_QUEUE);
    let Some(hook) = kbg_platform::start_input(tx) else {
        log::error!("no keyboard input: exiting");
        return;
    };
    let engine = std::thread::Builder::new()
        .name("kbg-engine".into())
        .spawn(move || run_engine(rx));
    let settings = Rc::new(RefCell::new(settings));
    let on_event = move |event| match event {
        TrayEvent::OpenSettings => open_settings(),
        TrayEvent::Reload => {
            let new = load_settings();
            kbg_platform::set_hotkeys(&new.hotkeys);
            kbg_platform::set_tray_menu(tray_menu(&i18n(&new)));
            log::info!("settings reloaded");
            *settings.borrow_mut() = new;
        }
        // No hotkeys while the settings window records one: its keys must reach it.
        TrayEvent::Recording(true) => kbg_platform::set_hotkeys(&Hotkeys {
            switch_layout: None,
            convert_last: None,
            convert_selection: None,
            change_case: None,
            toggle_enabled: None,
        }),
        TrayEvent::Recording(false) => kbg_platform::set_hotkeys(&settings.borrow().hotkeys),
    };
    kbg_platform::run_tray(menu, Rc::new(on_event));
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
