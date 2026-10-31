//! Read-only file mappings: language packs are used in place, and the OS pages in only what a
//! lookup touches. Mapped pages are shared with the file cache, not the process's private memory.
//! Windows keeps a mapped file from being replaced: a pack update goes to a new folder.

use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use kbg_core::Bytes;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_READONLY,
    UnmapViewOfFile,
};
use windows::core::PCWSTR;

struct Mapped {
    ptr: *const u8,
    len: usize,
}

// SAFETY: the view is read-only and lives until drop, so any thread may read it.
unsafe impl Send for Mapped {}
unsafe impl Sync for Mapped {}

impl AsRef<[u8]> for Mapped {
    fn as_ref(&self) -> &[u8] {
        // SAFETY: the view maps `len` readable bytes until drop.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl Drop for Mapped {
    fn drop(&mut self) {
        let view = MEMORY_MAPPED_VIEW_ADDRESS {
            Value: self.ptr as *mut c_void,
        };
        // SAFETY: the address came from MapViewOfFile and is unmapped once.
        let _ = unsafe { UnmapViewOfFile(view) };
    }
}

/// The whole file at `path`, mapped read-only.
pub fn map_file(path: &Path) -> std::io::Result<Bytes> {
    let file = std::fs::File::open(path)?;
    let len = usize::try_from(file.metadata()?.len()).map_err(std::io::Error::other)?;
    // Windows cannot map an empty file.
    if len == 0 {
        return Ok(Bytes::new(Vec::new()));
    }
    // SAFETY: the file handle is open for the calls; the view keeps the mapping alive after its
    // handle is closed, and the file may be closed once mapped.
    unsafe {
        let handle = HANDLE(file.as_raw_handle());
        let mapping = CreateFileMappingW(handle, None, PAGE_READONLY, 0, 0, PCWSTR::null())?;
        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        let _ = CloseHandle(mapping);
        if view.Value.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Bytes::new(Mapped {
            ptr: view.Value as *const u8,
            len,
        }))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn maps_a_file() {
        let path = std::env::temp_dir().join(format!("kbg-mapped-{}", std::process::id()));
        std::fs::write(&path, b"pack data").unwrap();
        let bytes = super::map_file(&path).unwrap();
        assert_eq!(bytes.as_ref(), b"pack data");
        drop(bytes);
        std::fs::remove_file(&path).unwrap();

        std::fs::write(&path, b"").unwrap();
        assert!(super::map_file(&path).unwrap().as_ref().is_empty());
        std::fs::remove_file(&path).unwrap();
        assert!(super::map_file(&path).is_err());
    }
}
