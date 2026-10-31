//! Everything that talks to the OS, behind one interface: the app, the settings window and the
//! engine never use `windows::` or `cfg(windows)` themselves. `windows/` is the real one;
//! `none.rs` lets the workspace build elsewhere (nothing works there yet).

#[cfg(not(windows))]
mod none;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use none::*;
#[cfg(windows)]
pub use windows::*;

/// The tray's texts, already translated.
#[derive(Clone)]
pub struct TrayMenu {
    pub tooltip: String,
    /// The on/off item (ticked while on).
    pub enabled: String,
    pub settings: String,
    pub exit: String,
}

/// What the tray asks of the app; handled on the tray's thread.
pub enum TrayEvent {
    OpenSettings,
    /// The settings window saved settings.gob.
    Reload,
    /// The settings window records a hotkey (true) or stopped (false): hotkeys rest meanwhile.
    Recording(bool),
}
