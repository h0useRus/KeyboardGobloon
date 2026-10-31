//! Keyboard and mouse input through low-level hooks on their own thread.
//!
//! The hook procedures only sort the event into an [`Input`] and hand it to the engine thread
//! through a bounded channel: Windows drops a hook that is slow to return. Keys still reach
//! the other programs, except the hotkeys we act on. Injected events (ours and anyone's) are
//! ignored, so what we type never comes back as input.

use std::cell::RefCell;
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;

use kbg_core::{Input, Key, Stroke};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_NUMLOCK, VK_PAUSE, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT,
    LLKHF_EXTENDED, LLKHF_INJECTED, MSG, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_QUIT, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
};

const SHIFT_L: u8 = 1;
const SHIFT_R: u8 = 2;
const CTRL_L: u8 = 4;
const CTRL_R: u8 = 8;
const ALT_L: u8 = 16;
/// Right Alt: AltGr on layouts that have it.
const ALT_R: u8 = 32;
const WIN_L: u8 = 64;
const WIN_R: u8 = 128;
/// Held with a key, these make it a shortcut rather than typing.
const SHORTCUT: u8 = CTRL_L | CTRL_R | ALT_L | WIN_L | WIN_R;

/// Scan code of the left Ctrl press Windows sends ahead of AltGr.
const ALTGR_FAKE_CTRL: u32 = 0x21D;

struct State {
    tx: SyncSender<Input>,
    mods: u8,
    caps: bool,
    /// A hotkey whose press we kept from the other programs: its release is kept too.
    swallowed: Option<VIRTUAL_KEY>,
    foreground: usize,
}

thread_local! {
    /// The hook procedures have no context argument; they run on the hook thread only.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn modifier(vk: VIRTUAL_KEY) -> Option<u8> {
    Some(match vk {
        VK_LSHIFT => SHIFT_L,
        VK_RSHIFT => SHIFT_R,
        VK_LCONTROL => CTRL_L,
        VK_RCONTROL => CTRL_R,
        VK_LMENU => ALT_L,
        VK_RMENU => ALT_R,
        VK_LWIN => WIN_L,
        VK_RWIN => WIN_R,
        _ => return None,
    })
}

/// Keys that neither type nor move the caret: function, lock and media keys.
fn ignored(vk: VIRTUAL_KEY) -> bool {
    matches!(vk, VK_NUMLOCK | VK_SCROLL) || matches!(vk.0, 0x70..=0x87 | 0xA6..=0xB7)
}

impl State {
    fn send(&self, input: Input) {
        // A full queue means the engine thread is stuck; dropping input is better than
        // blocking every keyboard in the system.
        let _ = self.tx.try_send(input);
    }

    /// Another window got the focus since the last key: the word belongs to the old one.
    fn check_focus(&mut self) {
        // SAFETY: no arguments, returns a handle value only.
        let foreground = unsafe { GetForegroundWindow() }.0 as usize;
        if foreground != self.foreground {
            self.foreground = foreground;
            self.send(Input::Break);
        }
    }

    /// Returns true when the key must not reach the other programs.
    fn key(&mut self, info: &KBDLLHOOKSTRUCT, down: bool) -> bool {
        let vk = VIRTUAL_KEY(info.vkCode as u16);
        if vk == VK_LCONTROL && info.scanCode == ALTGR_FAKE_CTRL {
            return false;
        }
        if let Some(bit) = modifier(vk) {
            if down {
                self.mods |= bit;
            } else {
                self.mods &= !bit;
            }
            return false;
        }
        if !down {
            if self.swallowed == Some(vk) {
                self.swallowed = None;
                return true;
            }
            return false;
        }
        if vk == VK_CAPITAL {
            self.caps = !self.caps;
            return false;
        }
        if ignored(vk) {
            return false;
        }
        self.check_focus();
        let input = if vk == VK_PAUSE && self.mods == 0 {
            self.swallowed = Some(vk);
            Input::ConvertLast
        } else if self.mods & SHORTCUT != 0 {
            Input::Break
        } else if vk == VK_BACK {
            Input::Backspace
        } else if vk == VK_SPACE {
            Input::Space
        } else {
            let key = Key {
                scan: (info.scanCode & 0xFF) as u16,
                extended: info.flags.0 & LLKHF_EXTENDED.0 != 0,
            };
            if key.is_text() {
                Input::Text(Stroke {
                    key,
                    shift: self.mods & (SHIFT_L | SHIFT_R) != 0,
                    caps: self.caps,
                    altgr: self.mods & ALT_R != 0,
                })
            } else {
                Input::Break
            }
        };
        self.send(input);
        input == Input::ConvertLast
    }
}

unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if info.flags.0 & LLKHF_INJECTED.0 == 0 {
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let swallow = STATE.with_borrow_mut(|s| s.as_mut().is_some_and(|s| s.key(info, down)));
            if swallow {
                return LRESULT(1);
            }
        }
    }
    // SAFETY: passing the event on is always allowed.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // A click may move the caret. Moves and the wheel return at once.
    if code == HC_ACTION as i32
        && matches!(
            wparam.0 as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        )
    {
        STATE.with_borrow(|s| {
            if let Some(s) = s {
                s.send(Input::Break);
            }
        });
    }
    // SAFETY: passing the event on is always allowed.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// The running hooks; dropping it removes them and closes the input channel.
pub struct InputHook {
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
}

/// Installs the hooks on a new thread; `None` when Windows refuses the keyboard hook.
pub fn start(tx: SyncSender<Input>) -> Option<InputHook> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("kbg-hook".into())
        .spawn(move || {
            // SAFETY: reads the toggle state only.
            let caps = unsafe { GetKeyState(VK_CAPITAL.0.into()) } & 1 != 0;
            STATE.set(Some(State {
                tx,
                mods: 0,
                caps,
                swallowed: None,
                foreground: 0,
            }));
            // SAFETY: the hooks are installed and removed on this thread, which runs the
            // message loop they need.
            unsafe {
                let module = GetModuleHandleW(None).ok().map(Into::into);
                let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), module, 0);
                let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), module, 0);
                let Ok(keyboard) = keyboard else {
                    log::error!("keyboard hook could not be installed");
                    let _ = ready_tx.send(None);
                    return;
                };
                if mouse.is_err() {
                    log::warn!("mouse hook could not be installed: clicks don't end the word");
                }
                let _ = ready_tx.send(Some(GetCurrentThreadId()));
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = UnhookWindowsHookEx(keyboard);
                if let Ok(mouse) = mouse {
                    let _ = UnhookWindowsHookEx(mouse);
                }
            }
            // Drops the sender: the engine thread ends.
            STATE.set(None);
        })
        .ok()?;
    match ready_rx.recv() {
        Ok(Some(thread_id)) => Some(InputHook {
            thread: Some(thread),
            thread_id,
        }),
        _ => {
            let _ = thread.join();
            None
        }
    }
}

impl Drop for InputHook {
    fn drop(&mut self) {
        // SAFETY: posting WM_QUIT ends the hook thread's message loop.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
