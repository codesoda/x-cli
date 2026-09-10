#!/bin/sh
# Install a checksum-verified macOS binary from codesoda/x-cli GitHub Releases.
set -eu

fail() { printf 'xcli install: %s\n' "$1" >&2; exit 1; }
[ "$(uname -s)" = Darwin ] || fail 'Prebuilt releases currently support macOS only'
case "$(uname -m)" in
  arm64|aarch64) target=aarch64-apple-darwin ;;
  x86_64) target=x86_64-apple-darwin ;;
  *) fail 'Unsupported architecture' ;;
esac
for tool in curl tar shasum mktemp; do
  command -v "$tool" >/dev/null 2>&1 || fail "Required tool missing: $tool"
done
repo=https://github.com/codesoda/x-cli
fetch() {
  curl -q --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
    --connect-timeout 15 --max-time 120 "$@"
}
version=${XCLI_VERSION:-}
if [ -z "$version" ]; then
  latest=$(fetch --output /dev/null --write-out '%{url_effective}' "$repo/releases/latest")
  version=${latest##*/}
fi
printf '%s\n' "$version" | LC_ALL=C grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || fail 'Expected a version such as v0.1.0'
prefix=${XCLI_INSTALL_DIR:-"${HOME:?HOME or XCLI_INSTALL_DIR must be set}/.local/bin"}
archive=xcli-$version-$target.tar.gz
work=$(mktemp -d)
staged=
cleanup() {
  rm -rf "$work"
  if [ -n "$staged" ]; then rm -f "$staged"; fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
fetch --output "$work/$archive" "$repo/releases/download/$version/$archive"
fetch --output "$work/checksums" "$repo/releases/download/$version/checksums-sha256.txt"
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
mkdir -p "$prefix"
[ ! -d "$prefix/xcli" ] || fail 'Installation destination is a directory'
staged=$(mktemp "$prefix/.xcli-install.XXXXXX")
cp "$work/xcli" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$prefix/xcli"
staged=
printf 'Installed xcli %s to %s/xcli\n' "${version#v}" "$prefix"
printf 'Ensure %s is on PATH.\n' "$prefix"
