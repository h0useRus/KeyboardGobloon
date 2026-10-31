//! Installed keyboard layouts (HKLs): which one a window types in, what keys type in each,
//! and switching the window to another.

use std::ffi::c_void;

use kbg_core::{LayoutId, Layouts, Stroke};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardLayoutList, HKL, MAPVK_VSC_TO_VK_EX, MapVirtualKeyExW,
    ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU, VK_SHIFT,
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
