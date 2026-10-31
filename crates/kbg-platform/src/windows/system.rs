//! System settings and helpers for the settings window: starting with Windows, the Windows
//! switch hotkey, keys held while recording a hotkey, bringing a window to the front.

use std::path::Path;

use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, FindWindowW, IsIconic, SW_RESTORE, SetForegroundWindow,
    ShowWindow,
};
use windows::core::{HSTRING, PCWSTR, w};

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
/// The value the installer and the settings window write (installer/KeyboardGobloon.iss).
const RUN_VALUE: PCWSTR = w!("KeyboardGobloon");

fn read_string(key: PCWSTR, value: PCWSTR) -> Option<String> {
    let mut buf = [0u16; 512];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: the buffer and its size in bytes match.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if result != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..len.min(buf.len())]))
}

/// Whether Keyboard Gobloon starts with Windows.
pub fn autostart() -> bool {
    read_string(RUN, RUN_VALUE).is_some()
}

/// Starts `exe` with Windows, or stops doing so.
pub fn set_autostart(exe: &Path, on: bool) -> bool {
    // SAFETY: registry writes of a string we own, with its size in bytes.
    let result: WIN32_ERROR = unsafe {
        if on {
            let command: Vec<u16> = format!("\"{}\"", exe.display())
                .encode_utf16()
                .chain([0])
                .collect();
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN,
                RUN_VALUE,
                REG_SZ.0,
                Some(command.as_ptr().cast()),
                (command.len() * 2) as u32,
            )
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, RUN_VALUE)
        }
    };
    result == ERROR_SUCCESS || (!on && !autostart())
}

/// The keys Windows itself switches layouts with, as text: Win+Space always, plus the
/// hotkey set in Windows' language settings (`Alt+Shift`, `Ctrl+Shift`, `` ` `` or none).
pub fn system_switch_keys() -> Vec<String> {
    let toggle = w!("Keyboard Layout\\Toggle");
    let setting = read_string(toggle, w!("Language Hotkey"))
        .or_else(|| read_string(toggle, w!("Hotkey")))
        .unwrap_or_else(|| "1".into());
    let mut keys = vec!["Win+Space".to_string()];
    match setting.trim() {
        "1" => keys.push("Alt+Shift".into()),
        "2" => keys.push("Ctrl+Shift".into()),
        "4" => keys.push("`".into()),
        _ => {}
    }
    keys
}

/// Virtual-key codes of every key held down now, left and right modifiers apart (for
/// recording a hotkey; egui knows no Pause key).
pub fn held_keys() -> Vec<u16> {
    (0x08u16..0xFF)
        // Mouse buttons, and the generic Shift/Ctrl/Alt (their sides are listed).
        .filter(|vk| !matches!(vk, 0x10..=0x12))
        // SAFETY: reads the key state only.
        .filter(|&vk| unsafe { GetAsyncKeyState(i32::from(vk)) } < 0)
        .collect()
}

/// Brings the window titled `title` to the front; false when there is none.
pub fn focus_window(title: &str) -> bool {
    // SAFETY: finding and activating a top-level window by title.
    unsafe {
        let Ok(window) = FindWindowW(None, &HSTRING::from(title)) else {
            return false;
        };
        if IsIconic(window).as_bool() {
            let _ = ShowWindow(window, SW_RESTORE);
        }
        let _ = SetForegroundWindow(window);
        true
    }
}

/// Lets a program we start next take the foreground (the user just clicked our tray).
pub fn allow_foreground() {
    // SAFETY: no arguments that matter.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}
