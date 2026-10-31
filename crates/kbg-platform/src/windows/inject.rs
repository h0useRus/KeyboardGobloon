//! Typing into the focused window with `SendInput`.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_BACK, VK_C, VK_LCONTROL,
    VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RETURN, VK_RMENU, VK_RSHIFT, VK_RWIN,
};

/// Marks our own events (`dwExtraInfo`), "KBG0".
const MARK: usize = 0x4B42_4730;
/// An unassigned key: pressing it between Alt (or Win) going down and up keeps that from
/// counting as a tap, which would open the window's menu (or Start).
const VK_NOTHING: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

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

fn press(inputs: &mut Vec<INPUT>, vk: VIRTUAL_KEY) {
    inputs.push(event(vk, 0, KEYBD_EVENT_FLAGS(0)));
    inputs.push(event(vk, 0, KEYEVENTF_KEYUP));
}

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    // SAFETY: the slice holds initialized keyboard INPUTs of the size given.
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        // Blocked by UIPI (an elevated window) or another program's input.
        log::warn!("SendInput sent {sent} of {} events", inputs.len());
    }
}

/// Types `text` as Unicode characters (the same in every layout); line breaks as Enter.
fn type_into(inputs: &mut Vec<INPUT>, text: &str) {
    for c in text.chars() {
        match c {
            '\r' => {}
            '\n' => press(inputs, VK_RETURN),
            _ => {
                let mut units = [0u16; 2];
                for &unit in c.encode_utf16(&mut units).iter() {
                    inputs.push(event(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
                    inputs.push(event(
                        VIRTUAL_KEY(0),
                        unit,
                        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    ));
                }
            }
        }
    }
}

/// Backspace `erase` times, then types `text`.
pub fn replace(erase: usize, text: &str) {
    let mut inputs = Vec::with_capacity(2 * (erase + text.len()));
    for _ in 0..erase {
        press(&mut inputs, VK_BACK);
    }
    type_into(&mut inputs, text);
    send(&inputs);
}

/// Types `text` over the selection (typing replaces it).
pub fn type_text(text: &str) {
    let mut inputs = Vec::with_capacity(2 * text.len());
    type_into(&mut inputs, text);
    send(&inputs);
}

/// Lets go of the modifiers the user still holds from the hotkey, so the keys we send next
/// are not shortcuts (Shift+Ctrl+C opens developer tools in browsers).
pub fn release_modifiers() {
    let held: Vec<VIRTUAL_KEY> = [
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
    ]
    .into_iter()
    // SAFETY: reads the key state only.
    .filter(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } < 0)
    .collect();
    if held.is_empty() {
        return;
    }
    let mut inputs = Vec::new();
    if held
        .iter()
        .any(|&vk| matches!(vk, VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN))
    {
        press(&mut inputs, VK_NOTHING);
    }
    for vk in held {
        inputs.push(event(vk, 0, KEYEVENTF_KEYUP));
    }
    send(&inputs);
}

/// Ctrl+C in the focused window.
pub fn copy() {
    let mut inputs = Vec::with_capacity(4);
    inputs.push(event(VK_LCONTROL, 0, KEYBD_EVENT_FLAGS(0)));
    press(&mut inputs, VK_C);
    inputs.push(event(VK_LCONTROL, 0, KEYEVENTF_KEYUP));
    send(&inputs);
}
