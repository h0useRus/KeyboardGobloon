mod hook;
mod icon;
mod inject;
mod instance;
mod layouts;
mod mapped;
mod selection;
mod system;
mod tray;

use std::path::PathBuf;

use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::System::SystemInformation::GetLocalTime;

pub use hook::{InputHook, enabled, set_enabled, set_hotkeys, start as start_input};
pub use inject::replace;
pub use instance::claim as claim_instance;
pub use layouts::{SystemLayouts, language_letters, layout_serves, switch as switch_layout};
pub use mapped::map_file;
pub use selection::transform as transform_selection;
pub use system::{
    allow_foreground, autostart, focus_window, held_keys, set_autostart, system_switch_keys,
};
pub use tray::{
    notify_app, notify_input, notify_recording, run as run_tray, set_menu as set_tray_menu,
};

/// `%APPDATA%\KeyboardGobloon`.
pub fn default_data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("KeyboardGobloon"))
}

/// Local time like `2026-10-31 14:03:27.512`.
pub fn local_time() -> String {
    // SAFETY: GetLocalTime only writes the returned struct.
    let t = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

/// The user's language code (`ru` from `ru-RU`).
pub fn user_language() -> Option<String> {
    // LOCALE_NAME_MAX_LENGTH
    let mut buf = [0u16; 85];
    // SAFETY: the buffer is as long as the slice says.
    let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
    let name = String::from_utf16_lossy(buf.get(..(len as usize).checked_sub(1)?)?);
    name.split('-').next().map(str::to_lowercase)
}
