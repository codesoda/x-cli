use super::*;
use crate::error::Kind;

#[test]
fn accepts_only_strict_stable_versions() {
    assert_eq!(parse_version("0.1.1").unwrap().to_string(), "0.1.1");
    assert_eq!(parse_version("10.20.30").unwrap().to_string(), "10.20.30");
    for raw in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "v1.2.3",
        "1.2.3-rc.1",
        "1.2.3+build",
        "01.2.3",
        "1..3",
        "1.2. 3",
        "1.2.-3",
        "1.2.99999999999",
    ] {
        assert!(parse_version(raw).is_none(), "{raw:?}");
    }
}

#[test]
fn comparison_is_numeric_not_lexicographic() {
    let v = |raw: &str| parse_version(raw).unwrap();
    assert!(v("0.9.0") < v("0.10.0"));
    assert!(v("1.10.0") > v("1.9.9"));
    assert!(v("2.0.0") > v("1.99.99"));
    assert_eq!(v("1.2.3"), v("1.2.3"));
}

#[test]
fn current_version_is_stable() {
    assert_eq!(
        current_version().unwrap().to_string(),
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn location_parsing_requires_stable_v_tags() {
    let (tag, version) =
        release_from_location("https://github.com/codesoda/x-cli/releases/tag/v1.2.3").unwrap();
    assert_eq!(tag, "v1.2.3");
    assert_eq!(version.to_string(), "1.2.3");
    let (tag, _) = release_from_location("/codesoda/x-cli/releases/tag/v0.2.0/").unwrap();
    assert_eq!(tag, "v0.2.0");
    for location in [
        "",
        "https://github.com/codesoda/x-cli/releases",
        "https://github.com/codesoda/x-cli/releases/tag/1.2.3",
        "https://github.com/codesoda/x-cli/releases/tag/v1.2.3-rc.1",
        "https://github.com/codesoda/x-cli/releases/tag/vlatest",
    ] {
        let error = release_from_location(location).unwrap_err();
        assert_eq!(error.kind, Kind::ProtocolChanged, "{location:?}");
    }
}

#[test]
fn only_native_macos_targets_are_supported() {
    assert_eq!(
        target_triple("macos", "aarch64").unwrap(),
        "aarch64-apple-darwin"
    );
    assert_eq!(
        target_triple("macos", "x86_64").unwrap(),
        "x86_64-apple-darwin"
    );
    for (os, arch) in [
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("windows", "x86_64"),
        ("macos", "arm"),
    ] {
        assert_eq!(target_triple(os, arch).unwrap_err().kind, Kind::Unsupported);
    }
}

#[test]
fn urls_and_asset_names_match_release_layout() {
    assert_eq!(
        asset_name("v1.2.3", "aarch64-apple-darwin"),
        "xcli-v1.2.3-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        latest_url(),
        "https://github.com/codesoda/x-cli/releases/latest"
    );
    assert_eq!(
        asset_url("v1.2.3", "xcli-v1.2.3-aarch64-apple-darwin.tar.gz"),
        "https://github.com/codesoda/x-cli/releases/download/v1.2.3/xcli-v1.2.3-aarch64-apple-darwin.tar.gz"
    );
    assert!(asset_url("v1.2.3", CHECKSUM_MANIFEST).ends_with("/checksums-sha256.txt"));
}

fn digest_of(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn checksum_verification_requires_exact_unambiguous_entry() {
    let asset = "xcli-v1.2.3-aarch64-apple-darwin.tar.gz";
    let digest = digest_of(b"archive");
    let manifest = format!(
        "{digest}  {asset}\n{}  xcli-v1.2.3-x86_64-apple-darwin.tar.gz\n",
        digest_of(b"other")
    );
    verify_checksum(b"archive", &manifest, asset).unwrap();
    // Binary-mode `*` prefix and uppercase digests still verify.
    let upper = format!("{}  *{asset}\n", digest.to_ascii_uppercase());
    verify_checksum(b"archive", &upper, asset).unwrap();

    let mismatch = verify_checksum(b"tampered", &manifest, asset).unwrap_err();
    assert_eq!(mismatch.kind, Kind::ProtocolChanged);

    let missing = verify_checksum(b"archive", "", asset).unwrap_err();
    assert_eq!(missing.kind, Kind::ProtocolChanged);

    let duplicated = format!("{digest}  {asset}\n{digest}  {asset}\n");
    assert_eq!(
        verify_checksum(b"archive", &duplicated, asset)
            .unwrap_err()
            .kind,
        Kind::ProtocolChanged
    );

    let malformed = format!("nothex  {asset}\n");
    assert_eq!(
        verify_checksum(b"archive", &malformed, asset)
            .unwrap_err()
            .kind,
        Kind::ProtocolChanged
    );

    // Extra fields make a line unrecognizable rather than trusted.
    let extra = format!("{digest}  {asset}  trailing\n");
    assert_eq!(
        verify_checksum(b"archive", &extra, asset).unwrap_err().kind,
        Kind::ProtocolChanged
    );
}
