use super::*;
use std::{mem::MaybeUninit, path::PathBuf};

#[cfg(test)]
mod tests;

fn mount_flags(file: &File) -> Result<libc::c_ulong> {
    let mut flags = MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: the descriptor is borrowed and flags points to writable storage.
    if unsafe { libc::fstatvfs(file.as_raw_fd(), flags.as_mut_ptr()) } != 0 {
        return Err(storage());
    }
    // SAFETY: successful fstatvfs initialized the structure.
    Ok(unsafe { flags.assume_init() }.f_flag)
}
fn sync_directory_link(parent: &File, child: &File, created: bool) -> Result<()> {
    sync_link_with(parent, child, created, mount_flags, File::sync_all)
}
fn sync_link_with(
    parent: &File,
    child: &File,
    created: bool,
    mut flags: impl FnMut(&File) -> Result<libc::c_ulong>,
    mut sync: impl FnMut(&File) -> io::Result<()>,
) -> Result<()> {
    // Reprocess every link on every creating walk, including after failed prior
    // creation. Readonly exemption is per existing namespace fd, NOT per pair.
    // It assumes stable topology and independently durable readonly namespaces;
    // ST_RDONLY alone does not establish persistent storage (see mutation-safety).
    for file in [child, parent] {
        if !created && flags(file)? & libc::ST_RDONLY != 0 {
            continue;
        }
        // Never forgive a failed sync, or qualify it by re-querying mount flags.
        // New pairs, file data and post-rename/unlink syncs have no exemption.
        sync(file).map_err(|_| storage())?;
    }
    Ok(())
}
fn create_directory(dir: &File, part: &CString) -> io::Result<()> {
    // SAFETY: valid borrowed descriptor and NUL-terminated component.
    if unsafe { mkdirat(dir.as_raw_fd(), part.as_ptr(), 0o700) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
pub(super) fn parent(path: &Path, create: bool) -> Result<Option<(File, CString)>> {
    parent_with_sync(path, create, sync_directory_link)
}
pub(super) fn parent_with_sync(
    path: &Path,
    create: bool,
    sync_link: impl FnMut(&File, &File, bool) -> Result<()>,
) -> Result<Option<(File, CString)>> {
    parent_with_ops(path, create, sync_link, create_directory)
}
fn parent_with_ops(
    path: &Path,
    create: bool,
    mut sync_link: impl FnMut(&File, &File, bool) -> Result<()>,
    mut mkdir: impl FnMut(&File, &CString) -> io::Result<()>,
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
        let (next, created) = match open(&dir, &part, DIRECTORY) {
            Ok(next) => (next, false),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                let created = match mkdir(&dir, &part) {
                    Ok(()) => true,
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => false,
                    Err(_) => return Err(storage()),
                };
                let next = open(&dir, &part, DIRECTORY).map_err(|_| storage())?;
                if created {
                    // Fix restrictive umasks, but never chmod an existing
                    // intermediate ancestor, including an EEXIST race winner.
                    private(&next, true)?;
                }
                (next, created)
            }
            Err(_) => return Err(storage()),
        };
        if create {
            sync_link(&dir, &next, created)?;
        }
        dir = next;
    }
    // Preserve repair of the final private state directory on all walks.
    private(&dir, true)?;
    Ok(Some((dir, filename)))
}
