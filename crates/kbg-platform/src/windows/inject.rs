//! Typing into the focused window with `SendInput`.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_BACK,
};

/// Marks our own events (`dwExtraInfo`), "KBG0".
const MARK: usize = 0x4B42_4730;

fn event(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARK,
            },
        },
    }
}

/// Backspace `erase` times, then types `text` as Unicode characters (the same in every layout).
pub fn replace(erase: usize, text: &str) {
    let mut inputs = Vec::with_capacity(2 * (erase + text.len()));
    for _ in 0..erase {
        inputs.push(event(VK_BACK, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(event(VK_BACK, 0, KEYEVENTF_KEYUP));
    }
    for unit in text.encode_utf16() {
        inputs.push(event(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
        inputs.push(event(
            VIRTUAL_KEY(0),
            unit,
            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        ));
    }
    // SAFETY: the slice holds initialized keyboard INPUTs of the size given.
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        // Blocked by UIPI (an elevated window) or another program's input.
        log::warn!("SendInput sent {sent} of {} events", inputs.len());
    }
}
