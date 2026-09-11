#!/bin/sh
# Install xcli to ~/.local/bin (or XCLI_INSTALL_DIR), either by downloading a
# checksum-verified macOS release binary or by building the x-cli checkout that
# contains this script. Run with --help for the mode and environment contract.
set -eu

repository=codesoda/x-cli
repo=https://github.com/$repository

fail() { printf 'xcli install: %s\n' "$1" >&2; exit 1; }

usage() {
  cat <<'EOF'
Install xcli.

Usage:
  curl -fsSL https://raw.githubusercontent.com/codesoda/x-cli/main/install.sh | sh
  sh install.sh [--release | --source | --help]

Modes:
  --release  Download the latest prebuilt macOS release, verify its SHA-256
             checksum and archive contents, and install it atomically. No
             cargo or GitHub login is required. This is the default when the
             script is piped or has no x-cli checkout beside it.
  --source   Build the x-cli checkout containing this script with
             `cargo build --release --locked` (warnings denied) and install
             the built binary. This is the default when an executed install.sh
             file has the x-cli Cargo.toml beside it. A piped script never
             selects source mode.

Environment:
  XCLI_INSTALL_DIR    Destination directory (default: ~/.local/bin).
  XCLI_VERSION        Pin a release tag such as v0.1.1. Release mode only;
                      it conflicts with source mode instead of being ignored.
  XCLI_DOWNLOAD_MODE  auto, curl or gh. Release mode only. auto and curl use
                      anonymous HTTPS downloads; gh uses an authenticated
                      GitHub CLI.
EOF
}

requested_mode=
for argument in "$@"; do
  case "$argument" in
    --help | -h)
      usage
      exit 0
      ;;
    --release | --source)
      wanted=${argument#--}
      if [ -n "$requested_mode" ] && [ "$requested_mode" != "$wanted" ]; then
        fail '--source and --release conflict; pass at most one mode'
      fi
      requested_mode=$wanted
      ;;
    *) fail "Unknown argument: $argument (see --help)" ;;
  esac
done

# Read a quoted key from the [package] section of a Cargo.toml.
manifest_field() {
  awk -F'"' -v key="$2" '
    /^[[:space:]]*\[/ { section = $0 }
    section ~ /^\[package\][[:space:]]*$/ && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" { print $2; exit }
  ' "$1"
}

# Source mode is only considered for an executed *.sh file with the x-cli
# Cargo.toml beside it. A piped script runs as "sh", which never matches, so
# the working directory can never turn a piped install into a source build.
checkout_dir=
foreign_manifest=
case "$0" in
  *.sh)
    if [ -f "$0" ]; then
      script_home=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P) ||
        fail 'Cannot resolve the directory containing install.sh'
      if [ -f "$script_home/Cargo.toml" ]; then
        if [ "$(manifest_field "$script_home/Cargo.toml" name)" = xcli ]; then
          checkout_dir=$script_home
        else
          foreign_manifest=$script_home/Cargo.toml
        fi
      fi
    fi
    ;;
esac

mode=$requested_mode
if [ -z "$mode" ]; then
  if [ -n "$checkout_dir" ]; then
    mode=source
  elif [ -n "$foreign_manifest" ]; then
    fail 'The Cargo.toml beside install.sh is not the xcli package; pass --release to install a prebuilt release'
  else
    mode=release
  fi
fi
if [ "$mode" = source ]; then
  [ -n "$checkout_dir" ] || fail 'Source mode requires executing an install.sh file with the x-cli Cargo.toml beside it'
  [ -z "${XCLI_VERSION:-}" ] || fail 'XCLI_VERSION conflicts with source mode; pass --release to install a pinned release'
  [ -z "${XCLI_DOWNLOAD_MODE:-}" ] || fail 'XCLI_DOWNLOAD_MODE conflicts with source mode; pass --release to download a release'
fi

prefix=${XCLI_INSTALL_DIR:-"${HOME:?HOME or XCLI_INSTALL_DIR must be set}/.local/bin"}
work=
staged=
cleanup() {
  if [ -n "$work" ]; then rm -rf "$work"; fi
  if [ -n "$staged" ]; then rm -f "$staged"; fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

fetch() {
  curl -q --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
    --connect-timeout 15 --max-time 120 "$@"
}

# Atomically replace $prefix/xcli with the verified binary at $1 (version $2).
install_binary() {
  mkdir -p "$prefix"
  [ ! -d "$prefix/xcli" ] || fail 'Installation destination is a directory'
  staged=$(mktemp "$prefix/.xcli-install.XXXXXX")
  cp "$1" "$staged"
  chmod 755 "$staged"
  mv -f "$staged" "$prefix/xcli"
  staged=
  printf 'Installed xcli %s to %s/xcli\n' "$2" "$prefix"
  printf 'Ensure %s is on PATH.\n' "$prefix"
}

install_from_source() {
  command -v cargo >/dev/null 2>&1 || fail 'Source mode requires cargo; install Rust or pass --release'
  command -v mktemp >/dev/null 2>&1 || fail 'Required tool missing: mktemp'
  expected=$(manifest_field "$checkout_dir/Cargo.toml" version)
  [ -n "$expected" ] || fail 'Cannot read the package version from Cargo.toml'
  # Pin the target directory (honoring an existing CARGO_TARGET_DIR) so the
  # built artifact is located reliably regardless of cargo configuration.
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    case "$CARGO_TARGET_DIR" in
      /*) target_dir=$CARGO_TARGET_DIR ;;
      *) target_dir=$checkout_dir/$CARGO_TARGET_DIR ;;
    esac
  else
    target_dir=$checkout_dir/target
  fi
  printf 'Building xcli %s from %s\n' "$expected" "$checkout_dir"
  (
    cd "$checkout_dir" &&
      CARGO_TARGET_DIR=$target_dir RUSTFLAGS="${RUSTFLAGS:-} -D warnings" \
        cargo build --release --locked
  ) || fail 'cargo build failed; the existing installation was not changed'
  built=$target_dir/release/xcli
  [ -f "$built" ] || fail 'Built binary is missing from the target directory'
  [ -s "$built" ] || fail 'Built binary is empty'
  [ "$("$built" --version)" = "xcli $expected" ] || fail 'Built binary cannot run or reports the wrong version'
  install_binary "$built" "$expected"
}

install_from_release() {
  [ "$(uname -s)" = Darwin ] || fail 'Prebuilt releases currently support macOS only; build an x-cli checkout with --source'
  case "$(uname -m)" in
    arm64 | aarch64) triple=aarch64-apple-darwin ;;
    x86_64) triple=x86_64-apple-darwin ;;
    *) fail 'Unsupported architecture' ;;
  esac
  for tool in tar shasum mktemp; do
    command -v "$tool" >/dev/null 2>&1 || fail "Required tool missing: $tool"
  done
  download=${XCLI_DOWNLOAD_MODE:-auto}
  # The repository is public: auto uses anonymous curl; gh remains explicit.
  [ "$download" != auto ] || download=curl
  case "$download" in
    gh) command -v gh >/dev/null 2>&1 || fail 'GitHub CLI is required for XCLI_DOWNLOAD_MODE=gh' ;;
    curl) command -v curl >/dev/null 2>&1 || fail 'curl is required for anonymous downloads' ;;
    *) fail 'XCLI_DOWNLOAD_MODE must be auto, gh or curl' ;;
  esac
  version=${XCLI_VERSION:-}
  if [ -z "$version" ]; then
    if [ "$download" = gh ]; then
      version=$(gh release view --repo "$repository" --json tagName --jq .tagName)
    else
      latest=$(fetch --output /dev/null --write-out '%{url_effective}' "$repo/releases/latest") ||
        fail 'Cannot resolve the latest release; check network access or set XCLI_VERSION'
      version=${latest##*/}
    fi
  fi
  printf '%s\n' "$version" | LC_ALL=C grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || fail 'Expected a version such as v0.1.0'
  archive=xcli-$version-$triple.tar.gz
  work=$(mktemp -d)
  if [ "$download" = gh ]; then
    gh release download "$version" --repo "$repository" --pattern "$archive" \
      --pattern checksums-sha256.txt --dir "$work" || fail 'Authenticated release download failed'
    mv "$work/checksums-sha256.txt" "$work/checksums"
  else
    fetch --output "$work/$archive" "$repo/releases/download/$version/$archive" ||
      fail 'Release download failed; check the version or set XCLI_DOWNLOAD_MODE=gh'
    fetch --output "$work/checksums" "$repo/releases/download/$version/checksums-sha256.txt" ||
      fail 'Checksum manifest download failed'
  fi
  # Verify exactly the requested asset, never unrelated entries or paths from the manifest.
  awk -v asset="$archive" '
    $2 == asset || $2 == "*" asset {
      if (length($1) != 64 || $1 ~ /[^0-9a-fA-F]/) exit 1
      print $1 "  " asset
      count++
    }
    END { if (count != 1) exit 1 }
  ' "$work/checksums" > "$work/check" || fail 'Missing or ambiguous asset checksum'
  (cd "$work" && shasum -a 256 -c check) || fail 'Checksum verification failed'
  [ "$(tar -tzf "$work/$archive")" = xcli ] || fail 'Unexpected archive contents'
  # Write only the named member to a fresh file; never extract archive paths or links.
  tar -xOzf "$work/$archive" xcli > "$work/xcli" || fail 'Cannot unpack release'
  [ -s "$work/xcli" ] || fail 'Release binary is empty'
  chmod 755 "$work/xcli"
  [ "$("$work/xcli" --version)" = "xcli ${version#v}" ] || fail 'Downloaded binary cannot run or has the wrong version'
  install_binary "$work/xcli" "${version#v}"
}

if [ "$mode" = source ]; then
  install_from_source
else
  install_from_release
fi
