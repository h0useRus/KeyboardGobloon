//! Technical log `KeyboardGobloon.log` next to the settings (`data_dir`; warnings, errors).
//! The previous run is kept as `KeyboardGobloon.old.log`. Messages are English, not translated,
//! and never contain typed text.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

struct FileLog(Mutex<File>);

impl Log for FileLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // Our crates in detail, libraries only when something is wrong.
        let target = metadata.target();
        let ours = target.starts_with("kbg_") || target.starts_with("KeyboardGobloon");
        metadata.level() <= if ours { Level::Debug } else { Level::Warn }
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {}: {}\r\n",
            kbg_platform::local_time(),
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut f) = self.0.lock() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.0.lock() {
            let _ = f.flush();
        }
    }
}

fn path() -> Option<PathBuf> {
    crate::data_dir().map(|dir| dir.join("KeyboardGobloon.log"))
}

/// Starts logging to the file; without a writable file nothing is logged.
pub fn init() {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::rename(&path, path.with_extension("old.log"));
    let Ok(file) = File::create(&path) else {
        return;
    };
    if log::set_boxed_logger(Box::new(FileLog(Mutex::new(file)))).is_ok() {
        log::set_max_level(LevelFilter::Debug);
    }
    log::info!("{} {} started", crate::APP_NAME, crate::VERSION);
}
