//! Offline installer fixtures: injected tool stubs, synthetic release assets,
//! synthetic checkouts, and helpers to run `install.sh` as a file or piped.
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::{fs, io};

pub const PREVIOUS_INSTALL: &str = "previous install";
pub const RELEASE_VERSION: &str = "0.1.0";
pub const SOURCE_VERSION: &str = "0.1.1";
const ARCHIVE: &str = "xcli-v0.1.0-x86_64-apple-darwin.tar.gz";

/// How the synthetic release assets should be shaped.
#[derive(Clone, Copy, PartialEq)]
pub enum Assets {
    Valid,
    BadChecksum,
    MissingEntry,
    ExtraMember,
    WrongVersion,
}

pub struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

fn repo_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh")
}

fn executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn write_tools(root: &Path) {
    let tools = root.join("tools");
    executable(
        &tools.join("uname"),
        "#!/bin/sh\nif [ \"$1\" = -s ]; then echo Darwin; else echo x86_64; fi\n",
    );
    executable(
        &tools.join("curl"),
        r#"#!/bin/sh
out=
previous=
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
printf '%s\n' "$*" >> "$XCLI_TOOL_LOG/gh"
case "$1:$2" in
  auth:status) exit 0 ;;
  release:view) echo v0.1.0 ;;
  release:download)
    previous=
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
        &tools.join("cargo"),
        r#"#!/bin/sh
{
  printf 'args=%s\n' "$*"
  printf 'rustflags=%s\n' "${RUSTFLAGS:-}"
  printf 'target=%s\n' "${CARGO_TARGET_DIR:-}"
  printf 'pwd=%s\n' "$(pwd -P)"
} >> "$XCLI_TOOL_LOG/cargo"
if [ -n "${XCLI_TEST_CARGO_FAIL:-}" ]; then exit 101; fi
mkdir -p "$CARGO_TARGET_DIR/release"
printf '#!/bin/sh\necho '\''xcli %s'\''\n' "$XCLI_TEST_BUILD_VERSION" \
  > "$CARGO_TARGET_DIR/release/xcli"
chmod 755 "$CARGO_TARGET_DIR/release/xcli"
"#,
    );
}

impl Fixture {
    /// Valid v0.1.0 release assets, stubbed tools, and a previous install.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for sub in ["tools", "assets", "installed", "bin", "log"] {
            fs::create_dir(root.join(sub)).unwrap();
        }
        write_tools(&root);
        let fixture = Fixture { _dir: dir, root };
        fixture.write_assets(Assets::Valid);
        fs::write(fixture.installed_binary(), PREVIOUS_INSTALL).unwrap();
        fs::copy(repo_script(), fixture.standalone_script()).unwrap();
        fixture
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A copy of install.sh with no Cargo.toml beside it.
    pub fn standalone_script(&self) -> PathBuf {
        self.root.join("bin/install.sh")
    }

    pub fn installed_binary(&self) -> PathBuf {
        self.root.join("installed/xcli")
    }

    /// A synthetic checkout: install.sh beside a Cargo.toml naming `package`.
    pub fn checkout(&self, name: &str, package: &str) -> PathBuf {
        let dir = self.root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::copy(repo_script(), dir.join("install.sh")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\nversion = \"{SOURCE_VERSION}\"\nedition = \"2024\"\n"),
        )
        .unwrap();
        dir
    }

    /// (Re)build the synthetic release archive and checksum manifest.
    pub fn write_assets(&self, mode: Assets) {
        let package = self.root.join("package");
        let _ = fs::remove_dir_all(&package);
        fs::create_dir(&package).unwrap();
        let reported = if mode == Assets::WrongVersion {
            "9.9.9"
        } else {
            RELEASE_VERSION
        };
        executable(
            &package.join("xcli"),
            &format!("#!/bin/sh\necho 'xcli {reported}'\n"),
        );
        let assets = self.root.join("assets");
        let mut tar = Command::new("tar");
        tar.arg("-czf")
            .arg(assets.join(ARCHIVE))
            .arg("-C")
            .arg(&package)
            .arg("xcli");
        if mode == Assets::ExtraMember {
            fs::write(package.join("extra"), "unexpected").unwrap();
            tar.arg("extra");
        }
        assert!(tar.status().unwrap().success());
        let digest = format!(
            "{:x}",
            Sha256::digest(fs::read(assets.join(ARCHIVE)).unwrap())
        );
        let manifest = match mode {
            Assets::BadChecksum => format!("{}  {ARCHIVE}\n", "0".repeat(64)),
            Assets::MissingEntry => format!("{digest}  unrelated.tar.gz\n"),
            _ => format!("{digest}  {ARCHIVE}\n"),
        };
        fs::write(assets.join("checksums-sha256.txt"), manifest).unwrap();
    }

    fn command(&self, envs: &[(&str, &str)]) -> Command {
        let path = std::env::join_paths(
            std::iter::once(self.root.join("tools"))
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        let mut command = Command::new("sh");
        command
            .env("PATH", path)
            .env("XCLI_FIXTURE_DIR", self.root.join("assets"))
            .env("XCLI_INSTALL_DIR", self.root.join("installed"))
            .env("XCLI_TOOL_LOG", self.root.join("log"))
            .env("XCLI_TEST_BUILD_VERSION", SOURCE_VERSION)
            .env_remove("XCLI_VERSION")
            .env_remove("XCLI_DOWNLOAD_MODE")
            .env_remove("XCLI_TEST_CARGO_FAIL")
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("RUSTFLAGS");
        for (key, value) in envs {
            command.env(key, value);
        }
        command
    }

    /// Execute an install.sh file directly, so `$0` is a real script path.
    pub fn run_file(&self, script: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut command = self.command(envs);
        command.arg(script).args(args);
        command.output().unwrap()
    }

    /// Pipe the repository install.sh into `sh` from `cwd`, as `curl | sh` does.
    pub fn run_piped(&self, cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut command = self.command(envs);
        if !args.is_empty() {
            command.arg("-s").arg("--").args(args);
        }
        command
            .current_dir(cwd)
            .stdin(Stdio::from(fs::File::open(repo_script()).unwrap()));
        command.output().unwrap()
    }

    /// Recorded invocations of a stubbed tool; empty when never called.
    pub fn tool_log(&self, tool: &str) -> String {
        match fs::read_to_string(self.root.join("log").join(tool)) {
            Ok(log) => log,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => panic!("unreadable {tool} log: {error}"),
        }
    }

    /// The `xcli --version` line reported by the installed binary.
    pub fn installed_version(&self) -> String {
        let output = Command::new(self.installed_binary())
            .arg("--version")
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    }

    /// Assert the previous installation survived and nothing was left staged.
    pub fn assert_untouched(&self) {
        assert_eq!(
            fs::read_to_string(self.installed_binary()).unwrap(),
            PREVIOUS_INSTALL
        );
        assert_eq!(
            fs::read_dir(self.root.join("installed")).unwrap().count(),
            1
        );
    }

    /// Assert a successful install left exactly one 0755 binary behind.
    pub fn assert_installed(&self, output: &Output, version: &str) {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::metadata(self.installed_binary())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            fs::read_dir(self.root.join("installed")).unwrap().count(),
            1
        );
        assert_eq!(self.installed_version(), format!("xcli {version}\n"));
    }
}
