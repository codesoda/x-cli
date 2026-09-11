//! Release metadata: strict stable-version parsing, numeric semantic
//! comparison, asset naming and unambiguous checksum-manifest verification.
use crate::error::{Error, Kind, Result};
use sha2::{Digest, Sha256};
use std::fmt;

pub(super) const CHECKSUM_MANIFEST: &str = "checksums-sha256.txt";
pub(super) const BINARY_NAME: &str = env!("CARGO_PKG_NAME");

/// Stable numeric semantic version. Field order gives numeric (never
/// lexicographic) derived ordering; pre-release and build metadata are
/// deliberately unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Accepts exactly `MAJOR.MINOR.PATCH` with plain decimal components and no
/// leading zeros; anything else (pre-releases, build metadata, prefixes) is
/// rejected so only stable releases are ever considered.
pub(super) fn parse_version(raw: &str) -> Option<Version> {
    let mut parts = raw.split('.');
    let version = Version {
        major: component(parts.next()?)?,
        minor: component(parts.next()?)?,
        patch: component(parts.next()?)?,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(version)
}
fn component(raw: &str) -> Option<u64> {
    if raw.is_empty()
        || raw.len() > 10
        || !raw.bytes().all(|b| b.is_ascii_digit())
        || (raw.len() > 1 && raw.starts_with('0'))
    {
        return None;
    }
    raw.parse().ok()
}

pub(super) fn current_version() -> Result<Version> {
    parse_version(env!("CARGO_PKG_VERSION")).ok_or_else(|| {
        Error::new(
            Kind::Unsupported,
            "This build does not report a stable semantic version; update is unavailable",
        )
    })
}

/// Extracts and strictly validates the release tag from the `releases/latest`
/// redirect location. Only stable `v`-prefixed semantic tags are accepted.
pub(super) fn release_from_location(location: &str) -> Result<(String, Version)> {
    let tag = location
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or_default();
    let version = tag
        .strip_prefix('v')
        .and_then(parse_version)
        .ok_or_else(|| {
            Error::new(
                Kind::ProtocolChanged,
                "Latest release tag is not a stable v-prefixed semantic version",
            )
        })?;
    Ok((tag.to_owned(), version))
}

/// Only native macOS release artifacts exist; everything else fails clearly.
pub(super) fn target_triple(os: &str, arch: &str) -> Result<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        _ => Err(Error::new(
            Kind::Unsupported,
            "No release artifact is published for this platform; reinstall from source instead",
        )),
    }
}

pub(super) fn asset_name(tag: &str, target: &str) -> String {
    format!("{BINARY_NAME}-{tag}-{target}.tar.gz")
}

pub(super) fn latest_url() -> String {
    format!("{}/releases/latest", env!("CARGO_PKG_REPOSITORY"))
}

pub(super) fn asset_url(tag: &str, asset: &str) -> String {
    format!(
        "{}/releases/download/{tag}/{asset}",
        env!("CARGO_PKG_REPOSITORY")
    )
}

/// Verifies the downloaded archive against the manifest entry for `asset`.
/// The manifest must contain exactly one well-formed entry for the asset.
pub(super) fn verify_checksum(archive: &[u8], manifest: &str, asset: &str) -> Result<()> {
    let expected = manifest_digest(manifest, asset)?;
    let actual = format!("{:x}", Sha256::digest(archive));
    if actual == expected {
        return Ok(());
    }
    Err(Error::new(
        Kind::ProtocolChanged,
        "Release archive does not match its published checksum",
    ))
}

fn manifest_digest(manifest: &str, asset: &str) -> Result<String> {
    let mut found: Option<String> = None;
    for line in manifest.lines() {
        let mut parts = line.split_whitespace();
        let (Some(digest), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        // sha256sum format has exactly two fields; `*` marks binary mode.
        if parts.next().is_some() || name.trim_start_matches('*') != asset {
            continue;
        }
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "Checksum manifest entry for the release asset is malformed",
            ));
        }
        if found.is_some() {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "Checksum manifest lists the release asset more than once",
            ));
        }
        found = Some(digest.to_ascii_lowercase());
    }
    found.ok_or_else(|| {
        Error::new(
            Kind::ProtocolChanged,
            "Checksum manifest has no entry for the release asset",
        )
    })
}

#[cfg(test)]
mod tests;
