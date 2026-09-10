#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn fixture(mode: &str) -> (tempfile::TempDir, std::process::Output) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let tools = root.join("tools");
    let assets = root.join("assets");
    let package = root.join("package");
    let install = root.join("installed");
    for path in [&tools, &assets, &package, &install] {
        fs::create_dir(path).unwrap();
    }
    executable(
        &tools.join("uname"),
        "#!/bin/sh\nif [ \"$1\" = -s ]; then echo Darwin; else echo x86_64; fi\n",
    );
    executable(
        &tools.join("curl"),
        r#"#!/bin/sh
out=
for arg do
  if [ "$previous" = --output ]; then out=$arg; fi
  previous=$arg
  url=$arg
done
if [ "$out" = /dev/null ]; then
  printf '%s\n' 'https://github.com/codesoda/x-cli/releases/tag/v0.1.0'
else
  cp "$XCLI_FIXTURE_DIR/${url##*/}" "$out"
fi
"#,
    );
    executable(
        &tools.join("gh"),
        r#"#!/bin/sh
case "$1:$2" in
  auth:status) exit 0 ;;
  release:view) echo v0.1.0 ;;
  release:download)
    for arg do
      if [ "$previous" = --dir ]; then destination=$arg; fi
      previous=$arg
    done
    cp "$XCLI_FIXTURE_DIR/"* "$destination/"
    ;;
  *) exit 90 ;;
esac
"#,
    );
    executable(
        &package.join("xcli"),
        if mode == "version" {
            "#!/bin/sh\necho 'xcli 9.9.9'\n"
        } else {
            "#!/bin/sh\necho 'xcli 0.1.0'\n"
        },
    );
    let archive = "xcli-v0.1.0-x86_64-apple-darwin.tar.gz";
    let mut tar = Command::new("tar");
    tar.arg("-czf")
        .arg(assets.join(archive))
        .arg("-C")
        .arg(&package)
        .arg("xcli");
    if mode == "archive" {
        fs::write(package.join("extra"), "unexpected").unwrap();
        tar.arg("extra");
    }
    assert!(tar.status().unwrap().success());
    let digest = format!(
        "{:x}",
        Sha256::digest(fs::read(assets.join(archive)).unwrap())
    );
    let manifest = match mode {
        "checksum" => format!("{}  {archive}\n", "0".repeat(64)),
        "missing" => format!("{digest}  unrelated.tar.gz\n"),
        _ => format!("{digest}  {archive}\n"),
    };
    fs::write(assets.join("checksums-sha256.txt"), manifest).unwrap();
    fs::write(install.join("xcli"), "previous install").unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = Command::new("sh")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh"))
        .env("PATH", path)
        .env("XCLI_FIXTURE_DIR", &assets)
        .env("XCLI_INSTALL_DIR", &install)
        .env(
            "XCLI_DOWNLOAD_MODE",
            match mode {
                "gh" => "gh",
                "auto" => "auto",
                _ => "curl",
            },
        )
        .env_remove("XCLI_VERSION")
        .output()
        .unwrap();
    (dir, output)
}

#[test]
fn installs_verified_release_and_resolves_latest_without_real_network() {
    let (dir, output) = fixture("ok");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let installed = dir.path().join("installed/xcli");
    assert_eq!(
        fs::metadata(&installed).unwrap().permissions().mode() & 0o777,
        0o755
    );
    let version = Command::new(installed).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(version.stdout, b"xcli 0.1.0\n");
}

#[test]
fn authenticated_and_auto_downloads_use_injected_github_cli() {
    for mode in ["gh", "auto"] {
        let (dir, output) = fixture(mode);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            Command::new(dir.path().join("installed/xcli"))
                .arg("--version")
                .status()
                .unwrap()
                .success()
        );
    }
}

#[test]
fn rejects_bad_checksum_manifest_archive_and_version_without_replacing_install() {
    for mode in ["checksum", "missing", "archive", "version"] {
        let (dir, output) = fixture(mode);
        assert!(!output.status.success(), "{mode}");
        assert_eq!(
            fs::read_to_string(dir.path().join("installed/xcli")).unwrap(),
            "previous install"
        );
        assert_eq!(
            fs::read_dir(dir.path().join("installed")).unwrap().count(),
            1
        );
    }
}
