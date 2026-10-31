//! The tray icon and its menu, on a hidden window that runs the main thread's message loop.

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, HICON, IDI_APPLICATION, LoadIconW, MF_STRING, MSG,
    PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_APP,
    WM_COMMAND, WM_DESTROY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::TrayMenu;

const WM_TRAY: u32 = WM_APP + 1;
const ID_EXIT: usize = 1;
/// The exe's icon resource (winresource gives the first icon id 1).
const APP_ICON: PCWSTR = PCWSTR(1 as _);

struct Tray {
    tooltip: Vec<u16>,
    exit: HSTRING,
    icon: HICON,
    /// Explorer broadcasts it after a restart: the icon must be added again.
    taskbar_created: u32,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn icon_data(window: HWND, tray: &Tray) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: tray.icon,
        ..Default::default()
    };
    let len = tray.tooltip.len().min(data.szTip.len() - 1);
    data.szTip[..len].copy_from_slice(&tray.tooltip[..len]);
    data
}

fn add_icon(window: HWND) {
    TRAY.with_borrow(|tray| {
        if let Some(tray) = tray {
            // SAFETY: the struct is fully initialized and outlives the call.
            if !unsafe { Shell_NotifyIconW(NIM_ADD, &icon_data(window, tray)) }.as_bool() {
                log::warn!("tray icon could not be added");
            }
        }
    });
}

fn show_menu(window: HWND) {
    let exit = TRAY.with_borrow(|t| t.as_ref().map(|t| t.exit.clone()));
    let Some(exit) = exit else { return };
    // SAFETY: the menu lives until DestroyMenu below; the label outlives TrackPopupMenu.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
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

unsafe extern "system" fn window_proc(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY if matches!(lparam.0 as u32, WM_RBUTTONUP | WM_LBUTTONUP) => show_menu(window),
        WM_COMMAND if wparam.0 & 0xFFFF == ID_EXIT => {
            // SAFETY: our own window, on its own thread.
            let _ = unsafe { DestroyWindow(window) };
        }
        WM_DESTROY => {
            TRAY.with_borrow(|tray| {
                if let Some(tray) = tray {
                    // SAFETY: as in add_icon.
                    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &icon_data(window, tray)) };
                }
            });
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
        }
        _ if TRAY.with_borrow(|t| t.as_ref().is_some_and(|t| t.taskbar_created == msg)) => {
            add_icon(window);
        }
        // SAFETY: default handling of every other message.
        _ => return unsafe { DefWindowProcW(window, msg, wparam, lparam) },
    }
    LRESULT(0)
}

/// Shows the tray icon and runs the message loop until the user picks Exit.
pub fn run(menu: &TrayMenu) {
    // SAFETY: the window class, window and icon are created and used on this thread only.
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let class = w!("KeyboardGobloon.tray");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: module.into(),
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            log::error!("tray window class could not be registered");
            return;
        }
        let icon = LoadIconW(Some(module.into()), APP_ICON)
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))
            .unwrap_or_default();
        TRAY.set(Some(Tray {
            tooltip: menu.tooltip.encode_utf16().collect(),
            exit: HSTRING::from(&menu.exit),
            icon,
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
        }));
        // Never shown: it only receives the tray icon's messages.
        let Ok(window) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("Keyboard Gobloon"),
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
        add_icon(window);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        TRAY.set(None);
    }
}
