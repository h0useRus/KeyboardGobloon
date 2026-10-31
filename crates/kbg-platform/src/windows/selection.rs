//! Changing the selected text in any window: copy it with Ctrl+C, change it, type the result
//! over the selection, and put the clipboard's text back as it was.

use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

use super::inject;

/// How long the window may take to put the selection on the clipboard.
const COPY_WAIT: Duration = Duration::from_millis(400);

/// The clipboard, open for as long as this lives. Other programs may hold it a moment: tries a
/// few times.
struct Clipboard;

impl Clipboard {
    fn open() -> Option<Self> {
        for _ in 0..10 {
            // SAFETY: opening without an owner window is allowed; closed on drop.
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Some(Self);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        log::warn!("clipboard busy: selection left alone");
        None
    }

    fn text(&self) -> Option<String> {
        // SAFETY: the clipboard is open; the handle's memory is locked while read and is a
        // null-terminated UTF-16 string for CF_UNICODETEXT.
        unsafe {
            IsClipboardFormatAvailable(u32::from(CF_UNICODETEXT.0)).ok()?;
            let handle = GetClipboardData(u32::from(CF_UNICODETEXT.0)).ok()?;
            let memory = HGLOBAL(handle.0);
            let ptr = GlobalLock(memory) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(memory);
            Some(text)
        }
    }

    fn set_text(&self, text: &str) {
        let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
        // SAFETY: the clipboard is open; the memory is filled before it is handed over, and
        // belongs to the clipboard afterwards.
        unsafe {
            let _ = EmptyClipboard();
            let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2) else {
                return;
            };
            let ptr = GlobalLock(memory) as *mut u16;
            if ptr.is_null() {
                return;
            }
            std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
            let _ = GlobalUnlock(memory);
            let _ = SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(memory.0)));
        }
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        // SAFETY: we opened it.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Copies the selection, gives it to `change` and types the result over it. Nothing happens
/// when nothing is selected, or `change` returns `None`. The clipboard's text is restored; other
/// clipboard contents (a picture) are replaced by the copied selection.
pub fn transform(change: impl FnOnce(&str) -> Option<String>) {
    inject::release_modifiers();
    let saved = Clipboard::open().map(|c| c.text());
    let Some(saved) = saved else { return };
    // SAFETY: a plain counter.
    let before = unsafe { GetClipboardSequenceNumber() };
    inject::copy();
    let start = Instant::now();
    // SAFETY: as above.
    while unsafe { GetClipboardSequenceNumber() } == before {
        if start.elapsed() > COPY_WAIT {
            // Nothing selected, or the window does not copy.
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let Some(selected) = Clipboard::open().and_then(|c| c.text()) else {
        return;
    };
    if let Some(changed) = change(&selected) {
        inject::type_text(&changed);
    }
    if let Some(clipboard) = Clipboard::open() {
        match &saved {
            Some(text) => clipboard.set_text(text),
            None => log::info!("clipboard held no text before: the copied selection stays on it"),
        }
    }
}
