//! Installed keyboard layouts (HKLs): which one a window types in, what keys type in each,
//! and switching the window to another.

use std::ffi::c_void;

use kbg_core::{Key, LayoutId, Layouts, Stroke};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Globalization::{GetLocaleInfoW, LOCALE_SISO639LANGNAME};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardLayoutList, HKL, MAPVK_VK_TO_VSC_EX, MAPVK_VSC_TO_VK_EX,
    MapVirtualKeyExW, ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU,
    VK_SHIFT, VkKeyScanExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};

/// ToUnicodeEx flag: leave the keyboard state (dead keys) alone (Windows 10 1607 and later).
const KEEP_STATE: u32 = 0x4;

fn hkl(layout: LayoutId) -> HKL {
    HKL(layout.0 as *mut c_void)
}

pub struct SystemLayouts;

impl Layouts for SystemLayouts {
    fn current(&self) -> Option<LayoutId> {
        // SAFETY: plain queries on the foreground window and its thread.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_invalid() {
                return None;
            }
            let thread = GetWindowThreadProcessId(window, None);
            Some(LayoutId(GetKeyboardLayout(thread).0 as usize))
        }
    }

    fn installed(&self) -> Vec<LayoutId> {
        // SAFETY: the first call counts, the second fills a buffer of that length.
        unsafe {
            let count = GetKeyboardLayoutList(None);
            let mut list = vec![HKL::default(); count.max(0) as usize];
            let count = GetKeyboardLayoutList(Some(&mut list));
            list.truncate(count.max(0) as usize);
            list.into_iter().map(|h| LayoutId(h.0 as usize)).collect()
        }
    }

    fn render(&self, layout: LayoutId, strokes: &[Stroke]) -> Option<String> {
        let layout = hkl(layout);
        let mut text = String::with_capacity(strokes.len() * 2);
        for stroke in strokes {
            let mut state = [0u8; 256];
            if stroke.shift {
                state[VK_SHIFT.0 as usize] = 0x80;
            }
            if stroke.caps {
                state[VK_CAPITAL.0 as usize] = 0x01;
            }
            if stroke.altgr {
                for vk in [VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU] {
                    state[vk.0 as usize] = 0x80;
                }
            }
            let scan = u32::from(stroke.key.scan) | if stroke.key.extended { 0xE000 } else { 0 };
            let mut buf = [0u16; 8];
            // SAFETY: the key state and buffer are the sizes the calls expect.
            let len = unsafe {
                let vk = MapVirtualKeyExW(scan, MAPVK_VSC_TO_VK_EX, Some(layout));
                if vk == 0 {
                    return None;
                }
                ToUnicodeEx(vk, scan, &state, &mut buf, KEEP_STATE, Some(layout))
            };
            // 0: the key types nothing here; below 0: a dead key.
            if len <= 0 {
                return None;
            }
            text.push_str(&String::from_utf16(buf.get(..len as usize)?).ok()?);
        }
        Some(text)
    }

    fn stroke(&self, layout: LayoutId, c: char) -> Option<Stroke> {
        // Characters outside the 16-bit range have no key.
        let unit = u16::try_from(u32::from(c)).ok()?;
        let layout = hkl(layout);
        // SAFETY: plain table lookups in the layout.
        let (scan_vk, scan) = unsafe {
            let found = VkKeyScanExW(unit, layout);
            if found == -1 {
                return None;
            }
            let vk = (found as u16) & 0xFF;
            (
                found as u16,
                MapVirtualKeyExW(u32::from(vk), MAPVK_VK_TO_VSC_EX, Some(layout)),
            )
        };
        // High byte: 1 Shift, 2 Ctrl, 4 Alt; Ctrl+Alt is AltGr. Ctrl or Alt alone types no text.
        let state = scan_vk >> 8;
        let altgr = state & 6 == 6;
        if state & 6 != 0 && !altgr || scan == 0 {
            return None;
        }
        Some(Stroke {
            key: Key {
                scan: (scan & 0xFF) as u16,
                extended: scan & 0xFF00 == 0xE000,
            },
            shift: state & 1 != 0,
            caps: false,
            altgr,
        })
    }
}

/// Two capital letters for the layout's language, as Windows shows it (`EN`, `RU`).
pub fn language_letters(layout: LayoutId) -> String {
    // The low word of an HKL is its language id.
    let lang = (layout.0 & 0xFFFF) as u32;
    let mut buf = [0u16; 9];
    // SAFETY: the buffer is as long as the slice says.
    let len = unsafe { GetLocaleInfoW(lang, LOCALE_SISO639LANGNAME, Some(&mut buf)) };
    let code = String::from_utf16_lossy(buf.get(..(len.max(1) as usize - 1)).unwrap_or(&[]));
    if code.is_empty() {
        return "??".into();
    }
    code.chars().take(2).collect::<String>().to_uppercase()
}

/// Whether `layout` types the language of one of `klids` (keyboard layout ids like `00000419`).
/// Only the main language counts, so English (Australia) uses a pack made for US layouts.
pub fn layout_serves(layout: LayoutId, klids: &[String]) -> bool {
    // The low word of an HKL and of a KLID is a language id; its low 10 bits the main language.
    let primary = |lang: usize| lang & 0x3FF;
    klids
        .iter()
        .filter_map(|k| usize::from_str_radix(k, 16).ok())
        .any(|k| primary(k & 0xFFFF) == primary(layout.0 & 0xFFFF))
}

/// Asks the foreground window to switch to `layout`, as the system switcher does.
pub fn switch(layout: LayoutId) {
    // SAFETY: posting a message to a window handle; a stale handle only fails the post.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return;
        }
        if PostMessageW(
            Some(window),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(layout.0 as isize),
        )
        .is_err()
        {
            log::warn!("layout switch request could not be posted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_packs_by_main_language() {
        let ru = ["00000419".to_string(), "00010419".to_string()];
        let en = ["00000409".to_string()];
        assert!(layout_serves(LayoutId(0x0419_0419), &ru));
        // English (Australia) with the US layout; Dvorak (a layout id in the high word).
        assert!(layout_serves(LayoutId(0x0409_0C09), &en));
        assert!(layout_serves(LayoutId(0xF002_0409), &en));
        assert!(!layout_serves(LayoutId(0x0409_0409), &ru));
        assert!(!layout_serves(LayoutId(0x0409_0409), &["junk".to_string()]));
    }
}
