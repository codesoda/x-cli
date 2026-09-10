# Public-read release verification

## Release contract

Release `v0.1.0` must provide downloadable Apple Silicon and Intel macOS binaries,
a checksum manifest, an installer, and evidence that the **downloaded and installed**
executables work. A source build or green unit suite alone is not completion.

The verified public surface is credential-free FxTwitter `read` and parent-only
`thread`, JSON/human output, ID/URL normalization, cache read/refresh/bypass/purge,
explicit partial-parent reporting, incompatible routing rejection and local
`doctor`. Authenticated search/timelines/replies are not claimed as verified public
features; search remains blocked in the observed account. No browser, Keychain,
account rotation or mutation is part of these release checks.

## Prompt-to-artifact checklist

| Requirement | Verification surface |
| --- | --- |
| Prove basic public ID and URL reads | `scripts/verify-public-release.py`: ID 20, X and Twitter URLs, normalized string IDs and expected public text |
| Prove root and nontrivial parent chains | Live chain `1903106713588568359` → `1903105387932295260` → `20`; assert parent edges, oldest-first ordering, root completeness, bounded partial exit 12 |
| JSON, human output, provenance/freshness | Parse actual CLI JSON and human text; assert FxTwitter/public scope, retrieval timestamp, cache state and no request failure |
| Cache controls and safe failures | Cache-hit URL variants, no-cache unchanged content files, refresh, TTL 0, purge/fresh read, invalid URL exit 2, incompatible account/provider exit 7 |
| Ship downloadable version | GitHub Release `v0.1.0`, versioned CHANGELOG notes, immutable tag, two native archives and checksums |
| Install downloaded version | Download the published `install.sh`; installer downloads native release archive, verifies checksum/version, installs executable into an isolated directory |
| Installed executable actually works | Run the same public verifier against the installed binary; record its SHA-256, version, time and passed checks |
| Gates cover release commit | Native stable/MSRV build/tests, format, Clippy, rustdoc, aislop gate 95, actionlint; release workflow invokes CI/aislop against resolved tag SHA |
| No accidental credential testing | Normal tests offline; release verifier always passes explicit FxTwitter where applicable, uses isolated state and never selects an account |

## Reproduce

Only run the live verifier explicitly; it contacts FxTwitter and stops on any
failure, including a rate limit. It never retries or changes account/provider.
Public fixture posts can become unavailable; that is a failed verification to
investigate, not an empty success to accept.

```sh
python3 scripts/verify-public-release.py \
  --binary /path/to/installed/xcli --expect-version 0.1.0 --live-public
```

Normal offline installer tests (`tests/installer.rs`) inject synthetic archives
and fake curl/uname commands into child processes, with no network access. They
cover checksum, manifest, archive and version rejection without replacing an
existing installation. They do not substitute for downloading the real release.

## Evidence so far

- Local source binary on macOS arm64: all 16 verifier checks passed against live
  FxTwitter on 2026-09-10. The nontrivial parent IDs were obtained from the public
  FxTwitter conversation for post 20, not from a browser session.
- Release publication and real downloaded-install evidence: pending until the
  release workflow and post-download checks complete. Do not treat the checklist
  above as proof that they have run.
