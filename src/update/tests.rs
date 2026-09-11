use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;

const CURRENT: &str = env!("CARGO_PKG_VERSION");

struct FakeReleases {
    location: Result<String>,
    assets: HashMap<String, Vec<u8>>,
    latest_calls: Cell<u32>,
    downloads: Cell<u32>,
}
impl FakeReleases {
    fn latest(tag: &str) -> Self {
        Self {
            location: Ok(format!(
                "https://github.com/codesoda/x-cli/releases/tag/{tag}"
            )),
            assets: HashMap::new(),
            latest_calls: Cell::new(0),
            downloads: Cell::new(0),
        }
    }
    fn failing(error: Error) -> Self {
        Self {
            location: Err(error),
            assets: HashMap::new(),
            latest_calls: Cell::new(0),
            downloads: Cell::new(0),
        }
    }
}
impl Releases for FakeReleases {
    fn latest_location(&self) -> Result<String> {
        self.latest_calls.set(self.latest_calls.get() + 1);
        self.location.clone()
    }
    fn download(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>> {
        self.downloads.set(self.downloads.get() + 1);
        let bytes = self
            .assets
            .get(url)
            .cloned()
            .ok_or_else(|| Error::new(Kind::Network, "Synthetic download failure"))?;
        assert!(bytes.len() as u64 <= max_bytes, "fixture exceeds bound");
        Ok(bytes)
    }
}

struct FakeHost {
    tty: bool,
    approve: bool,
    exe: PathBuf,
    reported: &'static str,
    confirms: Cell<u32>,
    prompts: RefCell<Vec<(String, String)>>,
}
impl FakeHost {
    fn new(exe: PathBuf) -> Self {
        Self {
            tty: true,
            approve: true,
            exe,
            reported: "xcli 9.9.9",
            confirms: Cell::new(0),
            prompts: RefCell::new(Vec::new()),
        }
    }
    fn non_tty() -> Self {
        let mut host = Self::new(PathBuf::from("/nonexistent/xcli"));
        host.tty = false;
        host
    }
}
impl Host for FakeHost {
    fn stdin_is_tty(&self) -> bool {
        self.tty
    }
    fn confirm(&self, current: &str, latest: &str) -> Result<bool> {
        assert!(self.tty, "confirm requires a terminal");
        self.confirms.set(self.confirms.get() + 1);
        self.prompts
            .borrow_mut()
            .push((current.to_owned(), latest.to_owned()));
        Ok(self.approve)
    }
    fn current_exe(&self) -> Result<PathBuf> {
        Ok(self.exe.clone())
    }
    fn probe_version(&self, binary: &Path) -> Result<String> {
        assert!(binary.exists(), "probe target must be staged");
        Ok(self.reported.to_owned())
    }
}

fn tar_gz(binary: &[u8]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_size(binary.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    builder.append_data(&mut header, "xcli", binary).unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

/// Fake releases serving a valid archive + manifest for Apple silicon.
fn release_fixture(tag: &str, binary: &[u8]) -> FakeReleases {
    let mut releases = FakeReleases::latest(tag);
    let asset = release::asset_name(tag, "aarch64-apple-darwin");
    let archive = tar_gz(binary);
    let manifest = format!("{:x}  {asset}\n", Sha256::digest(&archive));
    releases
        .assets
        .insert(release::asset_url(tag, &asset), archive);
    releases.assets.insert(
        release::asset_url(tag, release::CHECKSUM_MANIFEST),
        manifest.into_bytes(),
    );
    releases
}

fn install_fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().canonicalize().unwrap().join("xcli");
    fs::write(&exe, b"old-binary").unwrap();
    (dir, exe)
}

#[test]
fn check_is_non_mutating_and_reports_newer_release() {
    // --check works without a terminal and on unsupported platforms.
    let releases = FakeReleases::latest("v9.9.9");
    let status = run_with(
        true,
        false,
        "linux",
        "x86_64",
        &releases,
        &FakeHost::non_tty(),
    )
    .unwrap();
    assert_eq!(status["action"], "check");
    assert_eq!(status["current"], CURRENT);
    assert_eq!(status["latest"], "9.9.9");
    assert_eq!(status["update_available"], true);
    assert_eq!(releases.downloads.get(), 0);
    assert_eq!(status.get("installed"), None);
}

#[test]
fn check_reports_current_and_older_releases_without_updates() {
    let host = FakeHost::non_tty();
    let equal = FakeReleases::latest(&format!("v{CURRENT}"));
    let status = run_with(true, false, "macos", "aarch64", &equal, &host).unwrap();
    assert_eq!(status["update_available"], false);

    let older = FakeReleases::latest("v0.0.1");
    let status = run_with(true, false, "macos", "aarch64", &older, &host).unwrap();
    assert_eq!(status["latest"], "0.0.1");
    assert_eq!(status["update_available"], false);
}

#[test]
fn install_never_downgrades_or_reinstalls() {
    let host = FakeHost::non_tty();
    for tag in [format!("v{CURRENT}"), "v0.0.1".to_owned()] {
        let releases = FakeReleases::latest(&tag);
        let status = run_with(false, true, "macos", "aarch64", &releases, &host).unwrap();
        assert_eq!(status["action"], "install");
        assert_eq!(status["installed"], false);
        assert_eq!(status["reason"], "up_to_date");
        assert_eq!(releases.downloads.get(), 0, "{tag}");
    }
}

#[test]
fn non_tty_install_without_yes_fails_before_any_network_use() {
    let releases = FakeReleases::latest("v9.9.9");
    let error = run_with(
        false,
        false,
        "macos",
        "aarch64",
        &releases,
        &FakeHost::non_tty(),
    )
    .unwrap_err();
    assert_eq!(error.kind, Kind::InvalidInput);
    assert!(error.message.contains("--yes"));
    assert_eq!(releases.latest_calls.get(), 0);
    assert_eq!(releases.downloads.get(), 0);
}

#[test]
fn declined_prompt_cancels_before_download() {
    let (_dir, exe) = install_fixture();
    let releases = release_fixture("v9.9.9", b"new-binary");
    let mut host = FakeHost::new(exe.clone());
    host.approve = false;
    let status = run_with(false, false, "macos", "aarch64", &releases, &host).unwrap();
    assert_eq!(status["installed"], false);
    assert_eq!(status["reason"], "declined");
    assert_eq!(host.confirms.get(), 1);
    assert_eq!(
        host.prompts.borrow()[0],
        (CURRENT.to_owned(), "9.9.9".to_owned())
    );
    assert_eq!(releases.downloads.get(), 0);
    assert_eq!(fs::read(&exe).unwrap(), b"old-binary");
}

#[test]
fn confirmed_install_replaces_the_executable() {
    let (_dir, exe) = install_fixture();
    let releases = release_fixture("v9.9.9", b"new-binary");
    let host = FakeHost::new(exe.clone());
    let status = run_with(false, false, "macos", "aarch64", &releases, &host).unwrap();
    assert_eq!(host.confirms.get(), 1);
    assert_eq!(status["installed"], true);
    assert_eq!(status["path"], exe.display().to_string());
    assert_eq!(fs::read(&exe).unwrap(), b"new-binary");
}

#[test]
fn yes_flag_installs_without_prompting_even_without_a_terminal() {
    let (_dir, exe) = install_fixture();
    let releases = release_fixture("v9.9.9", b"new-binary");
    let mut host = FakeHost::new(exe.clone());
    host.tty = false;
    let status = run_with(false, true, "macos", "aarch64", &releases, &host).unwrap();
    assert_eq!(host.confirms.get(), 0);
    assert_eq!(status["installed"], true);
    assert_eq!(fs::read(&exe).unwrap(), b"new-binary");
}

#[test]
fn unsupported_platform_fails_clearly_before_download() {
    let (_dir, exe) = install_fixture();
    let releases = release_fixture("v9.9.9", b"new-binary");
    let host = FakeHost::new(exe.clone());
    let error = run_with(false, true, "linux", "x86_64", &releases, &host).unwrap_err();
    assert_eq!(error.kind, Kind::Unsupported);
    assert_eq!(releases.downloads.get(), 0);
    assert_eq!(fs::read(&exe).unwrap(), b"old-binary");
}

#[test]
fn checksum_mismatch_preserves_the_old_executable() {
    let (_dir, exe) = install_fixture();
    let tag = "v9.9.9";
    let mut releases = release_fixture(tag, b"new-binary");
    let asset = release::asset_name(tag, "aarch64-apple-darwin");
    releases.assets.insert(
        release::asset_url(tag, release::CHECKSUM_MANIFEST),
        format!("{:064x}  {asset}\n", 0).into_bytes(),
    );
    let error = run_with(
        false,
        true,
        "macos",
        "aarch64",
        &releases,
        &FakeHost::new(exe.clone()),
    )
    .unwrap_err();
    assert_eq!(error.kind, Kind::ProtocolChanged);
    assert_eq!(fs::read(&exe).unwrap(), b"old-binary");
}

#[test]
fn malformed_archive_and_version_mismatch_preserve_the_old_executable() {
    let (_dir, exe) = install_fixture();
    let tag = "v9.9.9";
    // Valid checksum over an archive without the expected root binary.
    let mut releases = FakeReleases::latest(tag);
    let asset = release::asset_name(tag, "aarch64-apple-darwin");
    let archive = b"not a tarball".to_vec();
    let manifest = format!("{:x}  {asset}\n", Sha256::digest(&archive));
    releases
        .assets
        .insert(release::asset_url(tag, &asset), archive);
    releases.assets.insert(
        release::asset_url(tag, release::CHECKSUM_MANIFEST),
        manifest.into_bytes(),
    );
    let host = FakeHost::new(exe.clone());
    let error = run_with(false, true, "macos", "aarch64", &releases, &host).unwrap_err();
    assert_eq!(error.kind, Kind::ProtocolChanged);

    // Staged binary reporting the wrong version is discarded.
    let releases = release_fixture(tag, b"new-binary");
    let mut host = FakeHost::new(exe.clone());
    host.reported = "xcli 0.0.1";
    let error = run_with(false, true, "macos", "aarch64", &releases, &host).unwrap_err();
    assert_eq!(error.kind, Kind::ProtocolChanged);
    assert_eq!(fs::read(&exe).unwrap(), b"old-binary");
    assert_eq!(
        fs::read_dir(exe.parent().unwrap()).unwrap().count(),
        1,
        "no staging leftovers"
    );
}

#[test]
fn network_and_malformed_release_errors_propagate_safely() {
    let host = FakeHost::non_tty();
    let offline = FakeReleases::failing(Error::new(Kind::Network, "Synthetic network failure"));
    assert_eq!(
        run_with(true, false, "macos", "aarch64", &offline, &host)
            .unwrap_err()
            .kind,
        Kind::Network
    );

    for tag in ["v9.9.9-rc.1", "latest", ""] {
        let releases = FakeReleases::latest(tag);
        let error = run_with(true, false, "macos", "aarch64", &releases, &host).unwrap_err();
        assert_eq!(error.kind, Kind::ProtocolChanged, "{tag:?}");
    }

    // Missing release asset surfaces the download failure.
    let (_dir, exe) = install_fixture();
    let releases = FakeReleases::latest("v9.9.9");
    let error = run_with(
        false,
        true,
        "macos",
        "aarch64",
        &releases,
        &FakeHost::new(exe.clone()),
    )
    .unwrap_err();
    assert_eq!(error.kind, Kind::Network);
    assert_eq!(fs::read(&exe).unwrap(), b"old-binary");
}

#[test]
fn human_rendering_covers_update_statuses_only() {
    let check = json!({
        "action": "check", "current": "0.1.1", "latest": "0.2.0",
        "update_available": true,
    });
    let text = human(&check).unwrap();
    assert!(text.contains("current: 0.1.1"));
    assert!(text.contains("newer release is available"));

    let current = json!({
        "action": "check", "current": "0.2.0", "latest": "0.2.0",
        "update_available": false,
    });
    assert!(human(&current).unwrap().contains("No newer release"));

    let installed = json!({
        "action": "install", "current": "0.1.1", "latest": "0.2.0",
        "update_available": true, "installed": true, "path": "/opt/bin/xcli",
    });
    let text = human(&installed).unwrap();
    assert!(text.contains("Updated xcli to 0.2.0 at /opt/bin/xcli"));

    let declined = json!({
        "action": "install", "current": "0.1.1", "latest": "0.2.0",
        "update_available": true, "installed": false, "reason": "declined",
    });
    assert!(human(&declined).unwrap().contains("declined"));

    let up_to_date = json!({
        "action": "install", "current": "0.2.0", "latest": "0.2.0",
        "update_available": false, "installed": false, "reason": "up_to_date",
    });
    assert!(
        human(&up_to_date)
            .unwrap()
            .contains("nothing was installed")
    );

    assert_eq!(human(&json!({"posts": []})), None);
    assert_eq!(human(&json!({"action": "purge"})), None);
}
