//! The tray icon and its menu, on a hidden window that runs the main thread's message loop.
//!
//! The icon shows the language of the window in front ([`super::icon`]). There is no timer
//! while nothing happens: the hooks post [`WM_CHECK`] after every key release and click, the
//! system tells us when another window comes to the front, and a short one-shot timer then
//! reads the layout (the window applies a switch a moment after the keys).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use kbg_core::Layouts;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, EVENT_SYSTEM_FOREGROUND, FindWindowW, GetCursorPos,
    GetMessageW, GetSystemMetrics, HICON, KillTimer, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    MF_UNCHECKED, MSG, PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
    SM_CXSMICON, SetForegroundWindow, SetTimer, TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WINEVENT_OUTOFCONTEXT, WM_APP, WM_COMMAND, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMER, WNDCLASSW,
    WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::{hook, icon, layouts};
use crate::{TrayEvent, TrayMenu};

const WM_TRAY: u32 = WM_APP + 1;
/// Something happened that may have changed the layout: look again shortly.
const WM_CHECK: u32 = WM_APP + 2;
const ID_ENABLED: usize = 1;
const ID_SETTINGS: usize = 2;
const ID_EXIT: usize = 3;
const TIMER_LAYOUT: usize = 1;
/// Time for the window to apply a layout switch before we read it.
const LAYOUT_DELAY_MS: u32 = 120;

/// The hidden window's class: the settings window finds the running app by it. Debug builds
/// have their own, like the single-instance mutex.
const CLASS: PCWSTR = if cfg!(debug_assertions) {
    w!("KeyboardGobloon.debug.tray")
} else {
    w!("KeyboardGobloon.tray")
};
/// Posted by the settings window after it saved settings.gob.
const RELOAD: PCWSTR = w!("KeyboardGobloon.reload");
/// Posted by the settings window while it records a hotkey (wparam 1) and after (0).
const RECORDING: PCWSTR = w!("KeyboardGobloon.recording");

/// What the icon shows now: letters, on/off and pixel size.
#[derive(PartialEq)]
struct Shown {
    letters: String,
    enabled: bool,
    size: i32,
}

struct Tray {
    menu: TrayMenu,
    on_event: Rc<dyn Fn(TrayEvent)>,
    icon: Option<HICON>,
    shown: Option<Shown>,
    added: bool,
    /// Explorer broadcasts it after a restart: the icon must be added again.
    taskbar_created: u32,
    reload: u32,
    recording: u32,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// The hidden window, for [`notify_input`] from other threads.
static WINDOW: AtomicUsize = AtomicUsize::new(0);

/// A key went up or the mouse clicked: the layout may have changed. Cheap, any thread.
pub fn notify_input() {
    let window = WINDOW.load(Ordering::Relaxed);
    if window != 0 {
        // SAFETY: posting to our own window; a closed one only fails the post.
        let _ = unsafe { PostMessageW(Some(HWND(window as _)), WM_CHECK, WPARAM(0), LPARAM(0)) };
    }
}

fn post_to_app(message: PCWSTR, wparam: usize) -> bool {
    // SAFETY: finding a window by class and posting it a registered message.
    unsafe {
        let Ok(window) = FindWindowW(CLASS, None) else {
            return false;
        };
        PostMessageW(
            Some(window),
            RegisterWindowMessageW(message),
            WPARAM(wparam),
            LPARAM(0),
        )
        .is_ok()
    }
}

/// From the settings window: tells the running app to read settings.gob again. False when
/// the app is not running.
pub fn notify_app() -> bool {
    post_to_app(RELOAD, 0)
}

/// From the settings window: the app's hotkeys rest while a new one is recorded, so the keys
/// reach the recorder.
pub fn notify_recording(on: bool) -> bool {
    post_to_app(RECORDING, usize::from(on))
}

/// New texts for the menu and tooltip (after a language change); call it on the tray thread,
/// from the event callback.
pub fn set_menu(menu: TrayMenu) {
    TRAY.with_borrow_mut(|t| {
        if let Some(t) = t {
            t.menu = menu;
            t.shown = None;
        }
    });
    let window = WINDOW.load(Ordering::Relaxed);
    if window != 0 {
        refresh(HWND(window as _));
    }
}

fn icon_data(window: HWND, icon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    let tip: Vec<u16> = tooltip.encode_utf16().collect();
    let len = tip.len().min(data.szTip.len() - 1);
    data.szTip[..len].copy_from_slice(&tip[..len]);
    data
}

/// Draws the icon again when the language, on/off or size changed.
fn refresh(window: HWND) {
    let letters = layouts::SystemLayouts
        .current()
        .map(layouts::language_letters);
    // SAFETY: a plain system metric.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    TRAY.with_borrow_mut(|tray| {
        let Some(tray) = tray else { return };
        // No window in front (the desktop): keep showing the last language.
        let letters = letters
            .or_else(|| tray.shown.as_ref().map(|s| s.letters.clone()))
            .unwrap_or_else(|| "??".into());
        let shown = Shown {
            letters,
            enabled: hook::enabled(),
            size,
        };
        if tray.shown.as_ref() == Some(&shown) && tray.added {
            return;
        }
        let Some(new_icon) = icon::language_icon(&shown.letters, size, shown.enabled) else {
            log::warn!("tray icon could not be drawn");
            return;
        };
        let mut tooltip = format!("{} · {}", tray.menu.tooltip, shown.letters);
        if !shown.enabled {
            tooltip.push_str(" · ⏸");
        }
        let data = icon_data(window, new_icon, &tooltip);
        let message = if tray.added { NIM_MODIFY } else { NIM_ADD };
        // SAFETY: the struct is fully initialized and outlives the call.
        if unsafe { Shell_NotifyIconW(message, &data) }.as_bool() {
            tray.added = true;
        } else if !tray.added {
            log::warn!("tray icon could not be added");
        }
        if let Some(old) = tray.icon.replace(new_icon) {
            // SAFETY: our own icon, no longer shown.
            let _ = unsafe { DestroyIcon(old) };
        }
        tray.shown = Some(shown);
    });
}

fn show_menu(window: HWND) {
    let labels = TRAY.with_borrow(|t| t.as_ref().map(|t| t.menu.clone()));
    let Some(labels) = labels else { return };
    let (enabled, settings, exit) = (
        HSTRING::from(&labels.enabled),
        HSTRING::from(&labels.settings),
        HSTRING::from(&labels.exit),
    );
    // SAFETY: the menu lives until DestroyMenu below; the labels outlive TrackPopupMenu.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let check = if hook::enabled() {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(menu, MF_STRING | check, ID_ENABLED, &enabled);
        let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, &settings);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, &exit);
        let mut at = POINT::default();
        let _ = GetCursorPos(&mut at);
        // Without this the menu does not close when the user clicks elsewhere.
        let _ = SetForegroundWindow(window);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            at.x,
            at.y,
            None,
            window,
            None,
        );
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}

/// Calls the app back outside the borrow of `TRAY` (it may call [`set_menu`]).
fn send_event(event: TrayEvent) {
    let handler = TRAY.with_borrow(|t| t.as_ref().map(|t| t.on_event.clone()));
    if let Some(handler) = handler {
        handler(event);
    }
}

unsafe extern "system" fn foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _window: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    notify_input();
}

unsafe extern "system" fn window_proc(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY if matches!(lparam.0 as u32, WM_RBUTTONUP | WM_LBUTTONUP) => show_menu(window),
        WM_CHECK => {
            // SAFETY: a one-shot timer on our own window; setting it again restarts it.
            let _ = unsafe { SetTimer(Some(window), TIMER_LAYOUT, LAYOUT_DELAY_MS, None) };
        }
        WM_TIMER if wparam.0 == TIMER_LAYOUT => {
            // SAFETY: our own timer.
            let _ = unsafe { KillTimer(Some(window), TIMER_LAYOUT) };
            refresh(window);
        }
        WM_SETTINGCHANGE | WM_DISPLAYCHANGE => {
            // The icon size may have changed.
            TRAY.with_borrow_mut(|t| {
                if let Some(t) = t {
                    t.shown = None;
                }
            });
            refresh(window);
        }
        WM_COMMAND => match wparam.0 & 0xFFFF {
            ID_ENABLED => hook::set_enabled(!hook::enabled()),
            ID_SETTINGS => send_event(TrayEvent::OpenSettings),
            ID_EXIT => {
                // SAFETY: our own window, on its own thread.
                let _ = unsafe { DestroyWindow(window) };
            }
            _ => {}
        },
        WM_DESTROY => {
            WINDOW.store(0, Ordering::Relaxed);
            TRAY.with_borrow(|tray| {
                if let Some(tray) = tray
                    && let Some(icon) = tray.icon
                {
                    // SAFETY: as in refresh.
                    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &icon_data(window, icon, "")) };
                }
            });
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
        }
        _ => {
            let (taskbar_created, reload, recording) = TRAY.with_borrow(|t| {
                t.as_ref()
                    .map_or((0, 0, 0), |t| (t.taskbar_created, t.reload, t.recording))
            });
            if msg != 0 && msg == taskbar_created {
                TRAY.with_borrow_mut(|t| {
                    if let Some(t) = t {
                        t.added = false;
                        t.shown = None;
                    }
                });
                refresh(window);
            } else if msg != 0 && msg == reload {
                send_event(TrayEvent::Reload);
                refresh(window);
            } else if msg != 0 && msg == recording {
                send_event(TrayEvent::Recording(wparam.0 != 0));
            } else {
                // SAFETY: default handling of every other message.
                return unsafe { DefWindowProcW(window, msg, wparam, lparam) };
            }
        }
    }
    LRESULT(0)
}

/// Shows the tray icon and runs the message loop until the user picks Exit. `on_event` runs on
/// this thread.
pub fn run(menu: TrayMenu, on_event: Rc<dyn Fn(TrayEvent)>) {
    // SAFETY: the window class, window, hook and icons are created and used on this thread.
    unsafe {
        // Sharp icons at every display scale (before any window exists).
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            log::error!("tray window class could not be registered");
            return;
        }
        TRAY.set(Some(Tray {
            menu,
            on_event,
            icon: None,
            shown: None,
            added: false,
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
            reload: RegisterWindowMessageW(RELOAD),
            recording: RegisterWindowMessageW(RECORDING),
        }));
        // Never shown: it only receives the tray icon's messages.
        let Ok(window) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            w!("Keyboard Gobloon tray"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module.into()),
            None,
        ) else {
            log::error!("tray window could not be created");
            return;
        };
        WINDOW.store(window.0 as usize, Ordering::Relaxed);
        let foreground = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(foreground_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        refresh(window);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if !foreground.is_invalid() {
            let _ = UnhookWinEvent(foreground);
        }
        TRAY.with_borrow_mut(|t| {
            if let Some(icon) = t.as_mut().and_then(|t| t.icon.take()) {
                let _ = DestroyIcon(icon);
            }
        });
        TRAY.set(None);
    }
}
