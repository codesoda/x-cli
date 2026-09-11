use super::*;
use crate::error::Kind;
use std::cell::RefCell;

fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (name, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, name, *bytes).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

#[test]
fn extracts_only_the_root_binary() {
    let archive = tar_gz(&[("README", b"docs"), ("xcli", b"binary-bytes")]);
    assert_eq!(extract_binary(&archive).unwrap(), b"binary-bytes");
    let dotted = tar_gz(&[("./xcli", b"dotted-bytes")]);
    assert_eq!(extract_binary(&dotted).unwrap(), b"dotted-bytes");
}

#[test]
fn rejects_archives_without_a_root_binary() {
    for archive in [
        tar_gz(&[]),
        tar_gz(&[("README", b"docs")]),
        tar_gz(&[("bin/xcli", b"nested")]),
        tar_gz(&[("xcli2", b"other")]),
        tar_gz(&[("xcli", b"")]),
    ] {
        let error = extract_binary(&archive).unwrap_err();
        assert_eq!(error.kind, Kind::ProtocolChanged);
    }
}

#[test]
fn rejects_corrupt_archives() {
    assert_eq!(
        extract_binary(b"not gzip data").unwrap_err().kind,
        Kind::ProtocolChanged
    );
    let mut truncated = tar_gz(&[("xcli", b"binary-bytes")]);
    truncated.truncate(truncated.len() / 2);
    assert!(extract_binary(&truncated).is_err());
}

struct FakeHost {
    exe: PathBuf,
    /// `--version` line the staged candidate pretends to report.
    reported: &'static str,
    probed: RefCell<Vec<PathBuf>>,
}
impl FakeHost {
    fn new(exe: PathBuf, reported: &'static str) -> Self {
        Self {
            exe,
            reported,
            probed: RefCell::new(Vec::new()),
        }
    }
}
impl Host for FakeHost {
    fn stdin_is_tty(&self) -> bool {
        false
    }
    fn confirm(&self, _: &str, _: &str) -> Result<bool> {
        panic!("replace_executable must not prompt")
    }
    fn current_exe(&self) -> Result<PathBuf> {
        Ok(self.exe.clone())
    }
    fn probe_version(&self, binary: &Path) -> Result<String> {
        self.probed.borrow_mut().push(binary.to_path_buf());
        Ok(self.reported.to_owned())
    }
}

fn install_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    (dir, root)
}

fn version(raw: &str) -> Version {
    release::parse_version(raw).unwrap()
}

#[test]
fn replaces_executable_atomically_after_version_probe() {
    let (_dir, root) = install_dir();
    let exe = root.join("xcli");
    fs::write(&exe, b"old-binary").unwrap();
    let host = FakeHost::new(exe.clone(), "xcli 0.2.0");

    let dest = replace_executable(&host, b"new-binary", &version("0.2.0")).unwrap();
    assert_eq!(dest, exe);
    assert_eq!(fs::read(&exe).unwrap(), b"new-binary");
    // The staged candidate, not the destination, was probed.
    let probed = host.probed.borrow();
    assert_eq!(probed.len(), 1);
    assert_ne!(probed[0], exe);
    assert_eq!(probed[0].parent().unwrap(), root);
    // No staging leftovers remain.
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&exe).unwrap().permissions().mode();
        assert_eq!(mode & 0o755, 0o755);
    }
}

#[cfg(unix)]
#[test]
fn resolves_symlinks_and_preserves_custom_destinations() {
    let (_dir, root) = install_dir();
    let target_dir = root.join("opt");
    fs::create_dir(&target_dir).unwrap();
    let target = target_dir.join("xcli");
    fs::write(&target, b"old-binary").unwrap();
    let link = root.join("xcli-link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let host = FakeHost::new(link.clone(), "xcli 0.2.0");

    let dest = replace_executable(&host, b"new-binary", &version("0.2.0")).unwrap();
    assert_eq!(dest, target);
    // The symlink survives and now resolves to the replaced binary.
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&link).unwrap(), b"new-binary");
    assert_eq!(fs::read(&target).unwrap(), b"new-binary");
}

#[test]
fn version_probe_mismatch_preserves_the_old_executable() {
    let (_dir, root) = install_dir();
    let exe = root.join("xcli");
    fs::write(&exe, b"old-binary").unwrap();
    for reported in ["xcli 0.1.9", "garbage", ""] {
        let host = FakeHost::new(exe.clone(), reported);
        let error = replace_executable(&host, b"new-binary", &version("0.2.0")).unwrap_err();
        assert_eq!(error.kind, Kind::ProtocolChanged, "{reported:?}");
    }
    assert_eq!(fs::read(&exe).unwrap(), b"old-binary");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}

#[test]
fn missing_destination_fails_without_writing() {
    let (_dir, root) = install_dir();
    let host = FakeHost::new(root.join("missing"), "xcli 0.2.0");
    let error = replace_executable(&host, b"new-binary", &version("0.2.0")).unwrap_err();
    assert_eq!(error.kind, Kind::Storage);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}

struct FailingProbeHost {
    exe: PathBuf,
}
impl Host for FailingProbeHost {
    fn stdin_is_tty(&self) -> bool {
        false
    }
    fn confirm(&self, _: &str, _: &str) -> Result<bool> {
        panic!("replace_executable must not prompt")
    }
    fn current_exe(&self) -> Result<PathBuf> {
        Ok(self.exe.clone())
    }
    fn probe_version(&self, _: &Path) -> Result<String> {
        Err(probe_error())
    }
}

#[test]
fn failed_probe_removes_staged_file_and_keeps_old_executable() {
    let (_dir, root) = install_dir();
    let exe = root.join("xcli");
    fs::write(&exe, b"old-binary").unwrap();
    let host = FailingProbeHost { exe: exe.clone() };
    let error = replace_executable(&host, b"new-binary", &version("0.2.0")).unwrap_err();
    assert_eq!(error.kind, Kind::ProtocolChanged);
    assert_eq!(fs::read(&exe).unwrap(), b"old-binary");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}

#[test]
fn system_host_probe_never_leaks_command_output_into_errors() {
    // A nonexistent candidate fails with the static reviewed message.
    let (_dir, root) = install_dir();
    let error = SystemHost
        .probe_version(&root.join("missing-binary"))
        .unwrap_err();
    assert_eq!(error.kind, Kind::ProtocolChanged);
    assert_eq!(
        error.message,
        "Downloaded executable failed the version check"
    );
}
