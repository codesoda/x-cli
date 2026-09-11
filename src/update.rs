//! Explicit self-update: `xcli update [--check] [-y|--yes]`.
//!
//! Updates never run automatically. `--check` is non-mutating; installation
//! requires an interactive confirmation or `--yes`, only ever upgrades, and
//! replaces the current executable atomically after checksum and version
//! validation. Network and host access go through seams so normal tests stay
//! fully offline. Update never touches accounts, caches or credentials.
mod install;
mod net;
mod release;
#[cfg(test)]
mod tests;

use crate::error::{Error, Kind, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Upper bound for a compressed release archive download.
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
/// Upper bound for the checksum manifest download.
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Network seam. The production implementation only talks HTTPS to the public
/// GitHub release endpoints with bounded redirects, time and size, and never
/// attaches credentials or X session material.
pub trait Releases {
    /// Resolve the `releases/latest` redirect target without following it.
    fn latest_location(&self) -> Result<String>;
    /// Download one release asset, following bounded HTTPS-only redirects.
    fn download(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>>;
}

/// Host seam for terminal interaction and executable replacement inputs.
pub trait Host {
    fn stdin_is_tty(&self) -> bool;
    /// Interactive confirmation; only called when stdin is a terminal.
    fn confirm(&self, current: &str, latest: &str) -> Result<bool>;
    /// Path of the executable to replace, before symlink resolution.
    fn current_exe(&self) -> Result<PathBuf>;
    /// `--version` output of a candidate executable. This runs the candidate,
    /// so callers must have verified the archive checksum first.
    fn probe_version(&self, binary: &Path) -> Result<String>;
}

/// Production entry point used by the CLI dispatcher.
pub fn run(check: bool, yes: bool) -> Result<Value> {
    let releases = net::GithubReleases::new()?;
    run_with(
        check,
        yes,
        std::env::consts::OS,
        std::env::consts::ARCH,
        &releases,
        &install::SystemHost,
    )
}

/// Dependency-injected update flow; normal tests never touch the network,
/// the real current executable, or a terminal.
pub(crate) fn run_with(
    check: bool,
    yes: bool,
    os: &str,
    arch: &str,
    releases: &dyn Releases,
    host: &dyn Host,
) -> Result<Value> {
    // Installing needs a confirmation channel; fail before any network use.
    if !check && !yes && !host.stdin_is_tty() {
        return Err(non_interactive());
    }
    let current = release::current_version()?;
    let location = releases.latest_location()?;
    let (tag, latest) = release::release_from_location(&location)?;
    let mut status = json!({
        "action": if check { "check" } else { "install" },
        "current": current.to_string(),
        "latest": latest.to_string(),
        "update_available": latest > current,
    });
    if check {
        return Ok(status);
    }
    // Equal or older releases are never installed; downgrades are refused.
    if latest <= current {
        status["installed"] = json!(false);
        status["reason"] = json!("up_to_date");
        return Ok(status);
    }
    if !yes {
        if !host.stdin_is_tty() {
            return Err(non_interactive());
        }
        if !host.confirm(&current.to_string(), &latest.to_string())? {
            status["installed"] = json!(false);
            status["reason"] = json!("declined");
            return Ok(status);
        }
    }
    let target = release::target_triple(os, arch)?;
    let asset = release::asset_name(&tag, target);
    let archive = releases.download(&release::asset_url(&tag, &asset), MAX_ARCHIVE_BYTES)?;
    let manifest = releases.download(
        &release::asset_url(&tag, release::CHECKSUM_MANIFEST),
        MAX_MANIFEST_BYTES,
    )?;
    let manifest = String::from_utf8(manifest).map_err(|_| {
        Error::new(
            Kind::ProtocolChanged,
            "Checksum manifest is not valid UTF-8",
        )
    })?;
    release::verify_checksum(&archive, &manifest, &asset)?;
    let binary = install::extract_binary(&archive)?;
    let path = install::replace_executable(host, &binary, &latest)?;
    status["installed"] = json!(true);
    status["path"] = json!(path.display().to_string());
    Ok(status)
}

/// Human rendering for update status JSON; `None` when the value is not one.
pub fn human(value: &Value) -> Option<String> {
    let action = value.get("action")?.as_str()?;
    if !matches!(action, "check" | "install") {
        return None;
    }
    let current = value.get("current")?.as_str()?;
    let latest = value.get("latest")?.as_str()?;
    let available = value.get("update_available")?.as_bool()?;
    let mut text = format!("current: {current}\nlatest: {latest}\n");
    match (action, value.get("installed").and_then(Value::as_bool)) {
        ("check", _) if available => {
            text.push_str("A newer release is available; run `xcli update` to install it.\n");
        }
        ("check", _) => text.push_str("No newer release is available.\n"),
        ("install", Some(true)) => {
            let path = value
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            text.push_str(&format!("Updated xcli to {latest} at {path}\n"));
        }
        ("install", _) => match value.get("reason").and_then(Value::as_str) {
            Some("declined") => text.push_str("Update declined; nothing was installed.\n"),
            _ => text.push_str("No newer release is available; nothing was installed.\n"),
        },
        _ => return None,
    }
    Some(text)
}

fn non_interactive() -> Error {
    Error::new(
        Kind::InvalidInput,
        "Interactive confirmation unavailable; rerun `xcli update --yes` to approve installation",
    )
}
