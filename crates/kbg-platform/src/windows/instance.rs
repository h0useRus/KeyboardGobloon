//! One running copy per user: a second start finds the named mutex and exits.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// Debug builds get their own name, so a development build runs next to an installed one.
/// The installer checks the release name (installer/KeyboardGobloon.iss, AppMutex).
const NAME: windows::core::PCWSTR = if cfg!(debug_assertions) {
    w!("Local\\KeyboardGobloon.debug.instance")
} else {
    w!("Local\\KeyboardGobloon.instance")
};

/// True for the first instance; the mutex stays open for the life of the process.
pub fn claim() -> bool {
    // SAFETY: plain kernel object calls; the handle is never closed on purpose.
    unsafe {
        let mutex = CreateMutexW(None, false, NAME);
        // Start anyway when the mutex fails.
        mutex.is_err() || GetLastError() != ERROR_ALREADY_EXISTS
    }
}
