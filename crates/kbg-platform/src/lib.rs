//! Everything that talks to the OS, behind one interface: the app and the engine never use
//! `windows::` or `cfg(windows)` themselves. `windows/` is the real one; `none.rs` lets the
//! workspace build elsewhere (nothing works there yet).

#[cfg(not(windows))]
mod none;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use none::*;
#[cfg(windows)]
pub use windows::*;

/// The tray icon's texts, already translated.
pub struct TrayMenu {
    pub tooltip: String,
    pub exit: String,
}
