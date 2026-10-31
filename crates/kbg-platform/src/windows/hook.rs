//! Keyboard and mouse input through low-level hooks on their own thread.
//!
//! The hook procedures only sort the event into an [`Input`] and hand it to the engine thread
//! through a bounded channel: Windows drops a hook that is slow to return. Keys still reach
//! the other programs, except the hotkeys we act on. Injected events (ours and anyone's) are
//! ignored, so what we type never comes back as input.
//!
//! Windows also removes a hook silently when it is slow once too often. A watchdog on the hook
//! thread compares the system's last input time with the last event the hooks saw and installs
//! them again when they fell silent.

use std::cell::RefCell;
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;
use std::time::Instant;

use kbg_core::{Input, Key, Stroke, Timings};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetLastInputInfo, LASTINPUTINFO, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_LCONTROL,
    VK_LMENU, VK_LSHIFT, VK_LWIN, VK_NUMLOCK, VK_PAUSE, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
    VK_SCROLL, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, HC_ACTION, HHOOK,
    KBDLLHOOKSTRUCT, KillTimer, LLKHF_EXTENDED, LLKHF_INJECTED, MSG, MSLLHOOKSTRUCT,
    PostThreadMessageW, SetTimer, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_QUIT,
    WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_TIMER, WM_XBUTTONDOWN,
};

/// How often the watchdog looks: one wake-up per 30 s costs nothing measurable.
const WATCHDOG_MS: u32 = 30_000;
/// Input the hooks did not see for this long means they were removed.
const SILENT_MS: u32 = 2_000;

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
    /// Time stamp of the last event either hook saw, injected ones included (tick count).
    last_seen: u32,
    /// How long the keyboard procedure takes for real keys; logged when the hooks stop.
    timings: Timings,
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
        let start = Instant::now();
        let swallow = STATE.with_borrow_mut(|s| {
            let Some(s) = s.as_mut() else { return false };
            s.last_seen = info.time;
            if info.flags.0 & LLKHF_INJECTED.0 != 0 {
                return false;
            }
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let swallow = s.key(info, down);
            s.timings.record(start.elapsed().as_nanos() as u64);
            swallow
        });
        if swallow {
            return LRESULT(1);
        }
    }
    // SAFETY: passing the event on is always allowed.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for WH_MOUSE_LL with HC_ACTION, lparam points to an MSLLHOOKSTRUCT.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        // A click may move the caret. Moves and the wheel only update the time stamp.
        let click = matches!(
            wparam.0 as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        );
        STATE.with_borrow_mut(|s| {
            if let Some(s) = s {
                s.last_seen = info.time;
                if click {
                    s.send(Input::Break);
                }
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

/// Both hooks, installed on the calling thread.
struct Hooks {
    keyboard: HHOOK,
    mouse: Option<HHOOK>,
}

impl Hooks {
    fn install(module: Option<HINSTANCE>) -> Option<Self> {
        // SAFETY: the procedures are valid for the life of the process; the thread that
        // installs them runs a message loop.
        unsafe {
            let Ok(keyboard) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), module, 0) else {
                log::error!("keyboard hook could not be installed");
                return None;
            };
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), module, 0).ok();
            if mouse.is_none() {
                log::warn!("mouse hook could not be installed: clicks don't end the word");
            }
            Some(Self { keyboard, mouse })
        }
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        // SAFETY: our own hooks, removed on the thread that installed them. Removing one that
        // Windows already dropped only fails.
        unsafe {
            let _ = UnhookWindowsHookEx(self.keyboard);
            if let Some(mouse) = self.mouse {
                let _ = UnhookWindowsHookEx(mouse);
            }
        }
    }
}

/// True when the system had input the hooks never saw.
fn hooks_silent() -> bool {
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: the struct is initialized with its size.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return false;
    }
    STATE.with_borrow_mut(|s| {
        let Some(s) = s.as_mut() else { return false };
        // Tick counts wrap after 49 days: compare the difference, not the values.
        let silent = info.dwTime.wrapping_sub(s.last_seen) as i32 > SILENT_MS as i32;
        if silent {
            s.last_seen = info.dwTime;
        }
        silent
    })
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
                last_seen: 0,
                timings: Timings::default(),
            }));
            // SAFETY: plain module and thread queries.
            let (module, thread_id) = unsafe {
                (
                    GetModuleHandleW(None).ok().map(Into::into),
                    GetCurrentThreadId(),
                )
            };
            let Some(mut hooks) = Hooks::install(module) else {
                let _ = ready_tx.send(None);
                return;
            };
            // The watchdog's first look starts from now.
            let _ = hooks_silent();
            let _ = ready_tx.send(Some(thread_id));
            // SAFETY: the timer and the message loop belong to this thread.
            unsafe {
                let timer = SetTimer(None, 0, WATCHDOG_MS, None);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_TIMER && msg.hwnd.is_invalid() {
                        if hooks_silent() {
                            // Also after typing on the lock screen or a UAC prompt, which
                            // no hook sees: installing again is harmless then.
                            log::info!("hooks saw no input for a while: installing them again");
                            drop(hooks);
                            let Some(again) = Hooks::install(module) else {
                                break;
                            };
                            hooks = again;
                        }
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = KillTimer(None, timer);
            }
            STATE.with_borrow(|s| {
                if let Some(s) = s {
                    log::info!("keyboard hook: {}", s.timings.summary());
                }
            });
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
