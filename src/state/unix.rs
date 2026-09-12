use super::*;
use std::{
    ffi::{CStr, CString, OsStr},
    fs::{File, Permissions},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, PermissionsExt},
        },
    },
    path::{Component, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

// Native constants and ABI signatures; every operation stays relative to
// pinned descriptors instead of re-resolving a checked path.
use libc::{
    O_CLOEXEC as CLOEXEC, O_CREAT as CREATE, O_DIRECTORY as DIRECTORY, O_EXCL as EXCLUSIVE,
    O_NOFOLLOW as NOFOLLOW, O_NONBLOCK as NONBLOCK, flock, geteuid, mkdirat, openat, renameat,
    unlinkat,
};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
mod tests;

fn name(s: &OsStr) -> Result<CString> {
    CString::new(s.as_bytes()).map_err(|_| storage())
}
fn open(dir: &File, name: &CString, flags: i32) -> io::Result<File> {
    // SAFETY: valid descriptor and NUL-terminated name; the returned fd is owned.
    let fd = unsafe {
        openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            flags | NOFOLLOW | CLOEXEC,
            0o600u32,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
fn private(file: &File, directory: bool) -> Result<()> {
    let meta = file.metadata().map_err(|_| storage())?;
    if meta.uid() != unsafe { geteuid() }
        || (directory && !meta.is_dir())
        || (!directory && (!meta.is_file() || meta.nlink() != 1))
    {
        return Err(storage());
    }
    file.set_permissions(Permissions::from_mode(if directory {
        0o700
    } else {
        0o600
    }))
    .map_err(|_| storage())
}
fn sync_created_directory(parent: &File, child: &File) -> Result<()> {
    // Persist the new directory/permissions, then its entry in the containing
    // directory, before creating any descendant state or lock file.
    child.sync_all().map_err(|_| storage())?;
    parent.sync_all().map_err(|_| storage())
}
fn parent(path: &Path, create: bool) -> Result<Option<(File, CString)>> {
    parent_with_sync(path, create, sync_created_directory)
}
fn parent_with_sync(
    path: &Path,
    create: bool,
    mut sync_created: impl FnMut(&File, &File) -> Result<()>,
) -> Result<Option<(File, CString)>> {
    let absolute: PathBuf = if path.is_absolute() {
        path.into()
    } else {
        std::env::current_dir().map_err(|_| storage())?.join(path)
    };
    let mut parts = Vec::new();
    for part in absolute.components() {
        match part {
            Component::RootDir | Component::CurDir => (),
            Component::Normal(s) => parts.push(name(s)?),
            _ => return Err(storage()),
        }
    }
    let filename = parts.pop().ok_or_else(storage)?;
    // Never chmod the filesystem root when passed a bare root-level filename.
    if parts.is_empty() {
        return Err(storage());
    }
    let mut dir = File::open("/").map_err(|_| storage())?;
    for part in parts {
        dir = match open(&dir, &part, DIRECTORY) {
            Ok(next) => next,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                let rc = unsafe { mkdirat(dir.as_raw_fd(), part.as_ptr(), 0o700) };
                if rc != 0 && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
                    return Err(storage());
                }
                let next = open(&dir, &part, DIRECTORY).map_err(|_| storage())?;
                // Fix restrictive umasks too (e.g. 0777).
                private(&next, true)?;
                sync_created(&dir, &next)?;
                next
            }
            Err(_) => return Err(storage()),
        };
    }
    private(&dir, true)?;
    Ok(Some((dir, filename)))
}
fn existing(dir: &File, filename: &CString) -> Result<Option<File>> {
    match open(dir, filename, NONBLOCK) {
        Ok(file) => {
            private(&file, false)?;
            Ok(Some(file))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(storage()),
    }
}
pub(super) fn remove_file(path: &Path) -> Result<()> {
    let Some((dir, filename)) = parent(path, false)? else {
        return Ok(());
    };
    if unsafe { unlinkat(dir.as_raw_fd(), filename.as_ptr(), 0) } != 0
        && io::Error::last_os_error().kind() != io::ErrorKind::NotFound
    {
        return Err(storage());
    }
    dir.sync_all().map_err(|_| storage())
}
struct DirectoryStream(*mut libc::DIR);
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        // SAFETY: fdopendir transferred one owned descriptor to this stream.
        unsafe {
            libc::closedir(self.0);
        }
    }
}
fn clear_errno() {
    // readdir uses null for both EOF and errors. POSIX requires clearing
    // thread-local errno before the call to distinguish the two.
    #[cfg(target_os = "linux")]
    unsafe {
        *libc::__errno_location() = 0;
    }
    #[cfg(target_os = "macos")]
    unsafe {
        *libc::__error() = 0;
    }
}
pub(super) fn prune_files(directory: &Path, keep: Option<&str>, max_files: usize) -> Result<()> {
    if let Some(keep) = keep {
        let mut components = Path::new(keep).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || max_files == 0
        {
            return Err(storage());
        }
    }
    // The synthetic filename is never opened. parent pins and secures only
    // the requested directory, not its existing ancestors.
    let Some((dir, _)) = parent(&directory.join(".entries"), false)? else {
        return Ok(());
    };
    let scan = open(&dir, &name(OsStr::new("."))?, DIRECTORY).map_err(|_| storage())?;
    // SAFETY: scan is an owned directory descriptor. Transfer ownership only
    // after fdopendir succeeds; otherwise File closes it on return.
    let raw = unsafe { libc::fdopendir(scan.as_raw_fd()) };
    if raw.is_null() {
        return Err(storage());
    }
    let _ = scan.into_raw_fd();
    let stream = DirectoryStream(raw);
    let mut entries = Vec::new();
    loop {
        clear_errno();
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            if io::Error::last_os_error().raw_os_error() != Some(0) {
                return Err(storage());
            }
            break;
        }
        // SAFETY: readdir's name is NUL terminated, valid until the next
        // readdir call. Copy it before advancing the stream.
        let entry = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_owned();
        if matches!(entry.to_bytes(), b"." | b"..") {
            continue;
        }
        let mut meta = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                dir.as_raw_fd(),
                entry.as_ptr(),
                meta.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            if io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                continue;
            }
            return Err(storage());
        }
        let meta = unsafe { meta.assume_init() };
        if !matches!(meta.st_mode & libc::S_IFMT, libc::S_IFREG | libc::S_IFLNK) {
            return Err(storage());
        }
        let protected = keep.is_some_and(|k| k.as_bytes() == entry.to_bytes());
        entries.push((protected, meta.st_mtime, entry));
    }
    entries.sort_by(|a, b| b.cmp(a));
    for (_, _, entry) in entries.into_iter().skip(max_files) {
        // No AT_REMOVEDIR: even if replaced concurrently with a directory,
        // unlink fails instead of traversing it. Symlinks lose only the link.
        if unsafe { unlinkat(dir.as_raw_fd(), entry.as_ptr(), 0) } != 0
            && io::Error::last_os_error().kind() != io::ErrorKind::NotFound
        {
            return Err(storage());
        }
    }
    dir.sync_all().map_err(|_| storage())
}
pub(super) fn with_lock<T>(path: &Path, action: impl FnOnce() -> Result<T>) -> Result<T> {
    let (dir, filename) = parent(path, true)?.ok_or_else(storage)?;
    // Separate exclusive creation from opening an existing lock. In
    // particular, concurrent O_CREAT|O_NOFOLLOW opens can report ENOENT on
    // Darwin. Retry only that bounded creation race, never symlink errors.
    let mut lock = None;
    for _ in 0..16 {
        match open(
            &dir,
            &filename,
            libc::O_RDWR | CREATE | EXCLUSIVE | NONBLOCK,
        ) {
            Ok(file) => {
                lock = Some(file);
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                match open(&dir, &filename, libc::O_RDWR | NONBLOCK) {
                    Ok(file) => {
                        lock = Some(file);
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                    Err(_) => return Err(storage()),
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(storage()),
        }
    }
    let lock = lock.ok_or_else(storage)?;
    private(&lock, false)?;
    loop {
        if unsafe { flock(lock.as_raw_fd(), libc::LOCK_EX) } == 0 {
            break;
        }
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(storage());
        }
    }
    let result = action();
    drop(lock); // Closing the descriptor releases the advisory lock, including on unwind.
    result
}
pub(super) fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let Some((dir, filename)) = parent(path, false)? else {
        return Ok(None);
    };
    let Some(file) = existing(&dir, &filename)? else {
        return Ok(None);
    };
    if file.metadata().map_err(|_| storage())?.len() > MAX_JSON_BYTES as u64 {
        return Err(storage());
    }
    let mut bytes = Vec::new();
    file.take(MAX_JSON_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| storage())?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(storage());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| storage())
}
pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let (dir, filename) = parent(path, true)?.ok_or_else(storage)?;
    existing(&dir, &filename)?;
    let (tempname, mut temp) = loop {
        let tempname = name(OsStr::new(&format!(
            ".state-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))?;
        match open(&dir, &tempname, libc::O_WRONLY | CREATE | EXCLUSIVE) {
            Ok(file) => break (tempname, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(storage()),
        }
    };
    let result = (|| {
        private(&temp, false)?;
        temp.write_all(bytes).map_err(|_| storage())?;
        temp.sync_all().map_err(|_| storage())?;
        existing(&dir, &filename)?;
        if unsafe {
            renameat(
                dir.as_raw_fd(),
                tempname.as_ptr(),
                dir.as_raw_fd(),
                filename.as_ptr(),
            )
        } != 0
        {
            return Err(storage());
        }
        dir.sync_all().map_err(|_| storage())
    })();
    // On failure remove the temporary entry without ever following it.
    if result.is_err() {
        unsafe {
            unlinkat(dir.as_raw_fd(), tempname.as_ptr(), 0);
        }
    }
    result
}
