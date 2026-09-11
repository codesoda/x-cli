//! Safe archive extraction and atomic executable replacement. Only the
//! expected root-level binary is ever extracted, the candidate must report
//! the expected version before it replaces anything, and every failure path
//! leaves the current executable untouched.
use super::{
    Host,
    release::{self, BINARY_NAME, Version},
};
use crate::error::{Error, Kind, Result};
use flate2::read::GzDecoder;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

/// Upper bound for the decompressed executable.
const MAX_BINARY_BYTES: u64 = 128 * 1024 * 1024;

/// Extracts exactly the root-level `xcli` regular file from a gzipped
/// tarball. Nothing else is ever written; directories, links and nested
/// paths are skipped, so archive path traversal is impossible.
pub(super) fn extract_binary(archive: &[u8]) -> Result<Vec<u8>> {
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    let entries = tar.entries().map_err(|_| corrupt_archive())?;
    for entry in entries {
        let entry = entry.map_err(|_| corrupt_archive())?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let matches = entry.path().map(|p| is_root_binary(&p)).unwrap_or(false);
        if !matches {
            continue;
        }
        let mut binary = Vec::new();
        entry
            .take(MAX_BINARY_BYTES + 1)
            .read_to_end(&mut binary)
            .map_err(|_| corrupt_archive())?;
        if binary.len() as u64 > MAX_BINARY_BYTES {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "Release executable exceeds the supported size",
            ));
        }
        if binary.is_empty() {
            return Err(corrupt_archive());
        }
        return Ok(binary);
    }
    Err(Error::new(
        Kind::ProtocolChanged,
        "Release archive does not contain the expected executable at its root",
    ))
}

fn is_root_binary(path: &Path) -> bool {
    let mut components = path
        .components()
        .filter(|component| !matches!(component, Component::CurDir));
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(name)), None) if name.to_str() == Some(BINARY_NAME)
    )
}

fn corrupt_archive() -> Error {
    Error::new(
        Kind::ProtocolChanged,
        "Release archive is corrupt or unreadable",
    )
}

/// Stages the verified binary next to the resolved current executable,
/// validates the version it reports, then renames it into place atomically.
/// Symlinked installs keep their symlink; the resolved target is replaced.
pub(super) fn replace_executable(
    host: &dyn Host,
    binary: &[u8],
    expected: &Version,
) -> Result<PathBuf> {
    let exe = host.current_exe()?;
    let dest = fs::canonicalize(&exe).map_err(|_| stage_error())?;
    let metadata = fs::symlink_metadata(&dest).map_err(|_| stage_error())?;
    if !metadata.is_file() {
        return Err(Error::new(
            Kind::Storage,
            "Current executable path does not resolve to a regular file",
        ));
    }
    let dir = dest.parent().ok_or_else(stage_error)?;
    let staged = stage(dir, binary)?;
    let swapped = validate_and_swap(host, &staged, &dest, expected);
    if swapped.is_err() {
        let _ = fs::remove_file(&staged);
    }
    swapped.map(|_| dest)
}

/// Writes the staged binary in the destination directory so the final rename
/// stays on one filesystem and is atomic.
fn stage(dir: &Path, binary: &[u8]) -> Result<PathBuf> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let path = dir.join(format!(
        ".{BINARY_NAME}-update-{}-{nanos}",
        std::process::id()
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o755);
    }
    let mut file = options.open(&path).map_err(|_| stage_error())?;
    let written = file.write_all(binary).and_then(|()| file.sync_all());
    drop(file);
    if written.is_err() {
        let _ = fs::remove_file(&path);
        return Err(stage_error());
    }
    Ok(path)
}

fn validate_and_swap(
    host: &dyn Host,
    staged: &Path,
    dest: &Path,
    expected: &Version,
) -> Result<()> {
    let reported = host.probe_version(staged)?;
    let version = reported
        .split_whitespace()
        .next_back()
        .and_then(release::parse_version)
        .ok_or_else(version_mismatch)?;
    if version != *expected {
        return Err(version_mismatch());
    }
    fs::rename(staged, dest).map_err(|_| {
        Error::new(
            Kind::Storage,
            "Cannot atomically replace the current executable",
        )
    })
}

fn stage_error() -> Error {
    Error::new(
        Kind::Storage,
        "Cannot stage the replacement executable next to the current one",
    )
}

fn version_mismatch() -> Error {
    Error::new(
        Kind::ProtocolChanged,
        "Downloaded executable does not report the expected release version",
    )
}

/// Production host: real terminal detection, stderr prompt, and a checked
/// `--version` probe of the staged (already checksum-verified) executable.
pub struct SystemHost;
impl Host for SystemHost {
    fn stdin_is_tty(&self) -> bool {
        use std::io::IsTerminal;
        std::io::stdin().is_terminal()
    }

    fn confirm(&self, current: &str, latest: &str) -> Result<bool> {
        let mut stderr = std::io::stderr().lock();
        write!(stderr, "Update xcli {current} -> {latest}? [y/N] ")
            .and_then(|()| stderr.flush())
            .map_err(|_| prompt_error())?;
        drop(stderr);
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|_| prompt_error())?;
        let line = line.trim();
        Ok(line.eq_ignore_ascii_case("y") || line.eq_ignore_ascii_case("yes"))
    }

    fn current_exe(&self) -> Result<PathBuf> {
        std::env::current_exe().map_err(|_| {
            Error::new(
                Kind::Storage,
                "Cannot determine the current executable path",
            )
        })
    }

    fn probe_version(&self, binary: &Path) -> Result<String> {
        let output = std::process::Command::new(binary)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|_| probe_error())?;
        if !output.status.success() {
            return Err(probe_error());
        }
        String::from_utf8(output.stdout)
            .map(|reported| reported.trim().to_owned())
            .map_err(|_| probe_error())
    }
}

fn prompt_error() -> Error {
    Error::new(
        Kind::InvalidInput,
        "Cannot read interactive confirmation; rerun `xcli update --yes` to approve installation",
    )
}

fn probe_error() -> Error {
    Error::new(
        Kind::ProtocolChanged,
        "Downloaded executable failed the version check",
    )
}

#[cfg(test)]
mod tests;
