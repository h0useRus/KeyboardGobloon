//! Stand-ins for systems without a backend yet: the app starts, sees no input and exits.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::SyncSender;

use kbg_core::{Input, LayoutId, Layouts, Stroke};
use kbg_settings::Hotkeys;

use crate::{TrayEvent, TrayMenu};

pub struct InputHook;

pub fn start_input(_tx: SyncSender<Input>) -> Option<InputHook> {
    None
}

pub fn set_hotkeys(_hotkeys: &Hotkeys) {}

pub fn enabled() -> bool {
    true
}

pub fn set_enabled(_on: bool) {}

pub struct SystemLayouts;

impl Layouts for SystemLayouts {
    fn current(&self) -> Option<LayoutId> {
        None
    }

    fn installed(&self) -> Vec<LayoutId> {
        Vec::new()
    }

    fn render(&self, _layout: LayoutId, _strokes: &[Stroke]) -> Option<String> {
        None
    }

    fn stroke(&self, _layout: LayoutId, _c: char) -> Option<Stroke> {
        None
    }
}

pub fn language_letters(_layout: LayoutId) -> String {
    "??".into()
}

pub fn layout_serves(_layout: LayoutId, _klids: &[String]) -> bool {
    false
}

pub fn map_file(path: &Path) -> std::io::Result<kbg_core::Bytes> {
    std::fs::read(path).map(kbg_core::Bytes::new)
}

pub fn replace(_erase: usize, _text: &str) {}

pub fn switch_layout(_layout: LayoutId) {}

pub fn transform_selection(_change: impl FnOnce(&str) -> Option<String>) {}

pub fn claim_instance() -> bool {
    true
}

pub fn default_data_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/KeyboardGobloon"))
}

pub fn local_time() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", now.as_secs(), now.subsec_millis())
}

pub fn user_language() -> Option<String> {
    None
}

pub fn run_tray(_menu: TrayMenu, _on_event: Rc<dyn Fn(TrayEvent)>) {}

pub fn set_tray_menu(_menu: TrayMenu) {}

pub fn notify_input() {}

pub fn notify_app() -> bool {
    false
}

pub fn notify_recording(_on: bool) -> bool {
    false
}

pub fn autostart() -> bool {
    false
}

pub fn set_autostart(_exe: &Path, _on: bool) -> bool {
    false
}

pub fn system_switch_keys() -> Vec<String> {
    Vec::new()
}

pub fn held_keys() -> Vec<u16> {
    Vec::new()
}

pub fn focus_window(_title: &str) -> bool {
    false
}

pub fn allow_foreground() {}
