//! Stand-ins for systems without a backend yet: the app starts, sees no input and exits.

use std::path::PathBuf;
use std::sync::mpsc::SyncSender;

use kbg_core::{Input, LayoutId, Layouts, Stroke};

use crate::TrayMenu;

pub struct InputHook;

pub fn start_input(_tx: SyncSender<Input>) -> Option<InputHook> {
    None
}

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
}

pub fn replace(_erase: usize, _text: &str) {}

pub fn switch_layout(_layout: LayoutId) {}

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

pub fn run_tray(_menu: &TrayMenu) {}
