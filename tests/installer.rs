#![cfg(unix)]
//! Offline synthetic tests for the dual-mode POSIX installer. Network tools,
//! `uname`, `gh`, and `cargo` are injected stubs; no real downloads or builds.
// Helpers live in tests/installer/ so cargo does not compile them as a
// standalone test crate; the path attribute keeps this file the crate root.
#[path = "installer/helpers.rs"]
mod helpers;

use helpers::{Assets, Fixture, RELEASE_VERSION, SOURCE_VERSION};

#[test]
fn standalone_script_installs_verified_release_with_anonymous_curl() {
    // Unset, auto, and explicit curl download modes all avoid gh and cargo.
    for envs in [
        &[][..],
        &[("XCLI_DOWNLOAD_MODE", "auto")][..],
        &[("XCLI_DOWNLOAD_MODE", "curl")][..],
    ] {
        let fixture = Fixture::new();
        let output = fixture.run_file(&fixture.standalone_script(), &[], envs);
        fixture.assert_installed(&output, RELEASE_VERSION);
        assert_eq!(fixture.tool_log("gh"), "", "auto must not probe gh");
        assert_eq!(fixture.tool_log("cargo"), "", "release mode must not build");
    }
}

#[test]
fn explicit_gh_mode_uses_injected_github_cli() {
    let fixture = Fixture::new();
    let output = fixture.run_file(
        &fixture.standalone_script(),
        &[],
        &[("XCLI_DOWNLOAD_MODE", "gh")],
    );
    fixture.assert_installed(&output, RELEASE_VERSION);
    assert!(fixture.tool_log("gh").contains("release download"));
    assert_eq!(fixture.tool_log("cargo"), "");
}

#[test]
fn piped_script_from_checkout_installs_release_without_cargo() {
    // `curl | sh` run inside an x-cli checkout must never infer source mode
    // from the working directory: it installs the release and skips cargo.
    let fixture = Fixture::new();
    let checkout = fixture.checkout("checkout", "xcli");
    let output = fixture.run_piped(&checkout, &[], &[]);
    fixture.assert_installed(&output, RELEASE_VERSION);
    assert_eq!(fixture.tool_log("cargo"), "");
    assert_eq!(fixture.tool_log("gh"), "");
}

#[test]
fn force_release_flag_overrides_source_checkout() {
    let fixture = Fixture::new();
    let checkout = fixture.checkout("checkout", "xcli");
    let output = fixture.run_file(&checkout.join("install.sh"), &["--release"], &[]);
    fixture.assert_installed(&output, RELEASE_VERSION);
    assert_eq!(fixture.tool_log("cargo"), "");
}

#[test]
fn source_mode_builds_checkout_with_spaces_locked_and_warnings_denied() {
    let fixture = Fixture::new();
    let checkout = fixture.checkout("check out dir", "xcli");
    let output = fixture.run_file(&checkout.join("install.sh"), &[], &[]);
    fixture.assert_installed(&output, SOURCE_VERSION);
    let log = fixture.tool_log("cargo");
    assert!(log.contains("args=build --release --locked"), "{log}");
    assert!(log.contains("-D warnings"), "{log}");
    assert!(
        log.contains(&format!("target={}", checkout.join("target").display())),
        "{log}"
    );
    assert!(
        log.contains(&format!("pwd={}", checkout.display())),
        "{log}"
    );
    assert_eq!(fixture.tool_log("gh"), "", "source mode must not download");
}

#[test]
fn source_mode_honors_preset_cargo_target_dir() {
    let fixture = Fixture::new();
    let checkout = fixture.checkout("checkout", "xcli");
    let target = fixture.root().join("custom target");
    let output = fixture.run_file(
        &checkout.join("install.sh"),
        &["--source"],
        &[("CARGO_TARGET_DIR", target.to_str().unwrap())],
    );
    fixture.assert_installed(&output, SOURCE_VERSION);
    assert!(
        fixture
            .tool_log("cargo")
            .contains(&format!("target={}", target.display()))
    );
    assert!(target.join("release/xcli").is_file());
}

#[test]
fn conflicting_or_invalid_modes_fail_before_building_or_installing() {
    // Version pin must not be silently ignored by a source build.
    let fixture = Fixture::new();
    let checkout = fixture.checkout("checkout", "xcli");
    let script = checkout.join("install.sh");
    let output = fixture.run_file(&script, &[], &[("XCLI_VERSION", "v0.1.0")]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("XCLI_VERSION"));
    fixture.assert_untouched();
    assert_eq!(fixture.tool_log("cargo"), "");

    // A download mode is meaningless for a source build; fail, do not ignore.
    let output = fixture.run_file(&script, &[], &[("XCLI_DOWNLOAD_MODE", "gh")]);
    assert!(!output.status.success());
    fixture.assert_untouched();

    // --source and --release are mutually exclusive.
    let output = fixture.run_file(&script, &["--source", "--release"], &[]);
    assert!(!output.status.success());
    fixture.assert_untouched();

    // A piped script has no checkout beside it, even when run from one.
    let output = fixture.run_piped(&checkout, &["--source"], &[]);
    assert!(!output.status.success());
    fixture.assert_untouched();
    assert_eq!(fixture.tool_log("cargo"), "");

    // --source without a Cargo.toml beside the script fails.
    let output = fixture.run_file(&fixture.standalone_script(), &["--source"], &[]);
    assert!(!output.status.success());
    fixture.assert_untouched();

    // Unknown arguments are rejected.
    let output = fixture.run_file(&fixture.standalone_script(), &["--bogus"], &[]);
    assert!(!output.status.success());
    fixture.assert_untouched();
}

#[test]
fn foreign_package_beside_script_fails_package_identity_validation() {
    let fixture = Fixture::new();
    let checkout = fixture.checkout("checkout", "impostor");
    for args in [&[][..], &["--source"][..]] {
        let output = fixture.run_file(&checkout.join("install.sh"), args, &[]);
        assert!(!output.status.success(), "{args:?}");
        fixture.assert_untouched();
        assert_eq!(fixture.tool_log("cargo"), "");
    }
}

#[test]
fn source_build_failure_or_wrong_version_retains_existing_install() {
    let fixture = Fixture::new();
    let checkout = fixture.checkout("checkout", "xcli");
    let script = checkout.join("install.sh");
    let output = fixture.run_file(&script, &[], &[("XCLI_TEST_CARGO_FAIL", "1")]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo build failed"));
    fixture.assert_untouched();

    let output = fixture.run_file(&script, &[], &[("XCLI_TEST_BUILD_VERSION", "9.9.9")]);
    assert!(!output.status.success());
    fixture.assert_untouched();
}

#[test]
fn rejects_bad_checksum_manifest_archive_and_version_without_replacing_install() {
    for assets in [
        Assets::BadChecksum,
        Assets::MissingEntry,
        Assets::ExtraMember,
        Assets::WrongVersion,
    ] {
        let fixture = Fixture::new();
        fixture.write_assets(assets);
        let output = fixture.run_file(&fixture.standalone_script(), &[], &[]);
        assert!(!output.status.success());
        fixture.assert_untouched();
    }
}

#[test]
fn help_prints_usage_without_touching_the_installation() {
    let fixture = Fixture::new();
    let output = fixture.run_piped(fixture.root(), &["--help"], &[]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in ["--release", "--source", "XCLI_VERSION", "XCLI_INSTALL_DIR"] {
        assert!(stdout.contains(expected), "missing {expected}");
    }
    fixture.assert_untouched();
    assert_eq!(fixture.tool_log("cargo"), "");
    assert_eq!(fixture.tool_log("gh"), "");
}
