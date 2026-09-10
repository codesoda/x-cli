//! Private, atomic JSON storage. No credentials belong in these files.
use crate::error::{Result, storage};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub(crate) const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    imp::read(path)
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    // Serialize before touching disk, including before creating a temporary file.
    struct Buffer(Vec<u8>);
    impl std::io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_JSON_BYTES - self.0.len() {
                return Err(std::io::Error::other("Local state exceeds size limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = Buffer(Vec::new());
    serde_json::to_writer(&mut bytes, value).map_err(|_| storage())?;
    imp::write(path, &bytes.0)
}

/// Serialize read/modify/write operations across processes using a separate,
/// never-renamed lock inode. This is not a credential store.
pub(crate) fn with_lock<T>(path: &Path, action: impl FnOnce() -> Result<T>) -> Result<T> {
    imp::with_lock(path, action)
}

/// Unlink one entry relative to its pinned parent. Missing entries are harmless;
/// symlinks themselves are removed and directories are never traversed.
pub(crate) fn remove_file(path: &Path) -> Result<()> {
    imp::remove_file(path)
}

/// Prune a private flat directory, retaining at most `max_files` newest entries.
/// `keep`, when provided, is a single filename protected from eviction. With a
/// zero limit this purges all entries. Symlinks are unlinked, never followed;
/// unexpected subdirectories are rejected rather than traversed.
pub(crate) fn prune_files(directory: &Path, keep: Option<&str>, max_files: usize) -> Result<()> {
    imp::prune_files(directory, keep, max_files)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use unix as imp;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod imp {
    use super::*;
    // Fail closed on platforms where the secure directory-relative implementation
    // has not been audited.
    pub(super) fn remove_file(_: &Path) -> Result<()> {
        Err(storage())
    }
    pub(super) fn prune_files(_: &Path, _: Option<&str>, _: usize) -> Result<()> {
        Err(storage())
    }
    pub(super) fn with_lock<T>(_: &Path, _: impl FnOnce() -> Result<T>) -> Result<T> {
        Err(storage())
    }
    pub(super) fn read<T: DeserializeOwned>(_: &Path) -> Result<Option<T>> {
        Err(storage())
    }
    pub(super) fn write(_: &Path, _: &[u8]) -> Result<()> {
        Err(storage())
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests;
