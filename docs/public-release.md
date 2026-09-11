# Public-read release verification

## Release contract

Release `v0.1.1` must provide downloadable Apple Silicon and Intel macOS binaries,
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
| Ship downloadable version | GitHub Release `v0.1.1`, versioned CHANGELOG notes, immutable tag, two native archives and checksums |
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
  --binary /path/to/installed/xcli --expect-version 0.1.1 --live-public
```

Normal offline installer tests (`tests/installer.rs`) inject synthetic archives
and fake `uname`/`curl`/`gh`/`cargo` commands into child processes, with no
network access. They cover release and source install modes plus checksum,
manifest, archive and version rejection without replacing an existing
installation. They do not substitute for downloading the real release.

## Completion audit — verified 2026-09-10

- Local source binary on macOS arm64: all 16 verifier checks passed against live
  FxTwitter on 2026-09-10. The nontrivial parent IDs were obtained from the public
  FxTwitter conversation for post 20, not from a browser session.
- `v0.1.0` published successfully, but both post-download jobs failed at anonymous
  installer download with HTTP 404. Inspection confirmed the repository was
  private at the time. This was a real installer defect for this repository as
  then configured, not successful verification.
- **`v0.1.1` is published**, not a draft or prerelease:
  https://github.com/codesoda/x-cli/releases/tag/v0.1.1 . Repository visibility
  was private at verification time, so authenticated GitHub access was required
  for these downloads. The repository has since been made public (confirmed via
  the anonymous GitHub API on 2026-09-11, `"private": false`); no anonymous
  re-download of the v0.1.1 assets has been performed or is claimed here.
- Tag commit: `77a97e3196d51261504e0429f7224ee2f51f6ec0`, merged through
  [PR #3](https://github.com/codesoda/x-cli/pull/3), following the initial
  [PR #2](https://github.com/codesoda/x-cli/pull/2).
- [Release run 34540909288](https://github.com/codesoda/x-cli/actions/runs/34540909288)
  passed every job: tag/notes validation, reusable CI and aislop gates, native
  release builds/tests, publication, and both post-publication download/install/
  public-read verification jobs. The installer/binaries were downloaded from
  the release, not reused from build outputs.
- Both native runners passed **all 16** verifier checks on installed binaries.
  The published installer was also independently downloaded and run locally on
  macOS 26.2 arm64, followed by the same 16 live checks. No browser/Keychain access.
- All three release-file checksums (both archives and installer) matched the
  downloaded manifest. Archive hashes also matched GitHub's asset digests.
  Extracted executable hashes matched the installed binary and the corresponding
  native verifier reports, establishing that the tested binaries are the shipped
  assets. Each archive contained only the expected `xcli` member.
- The unpinned latest/auto installer path was additionally exercised locally. It
  selected v0.1.1, installed the same arm64 binary hash, and passed a live read of
  post 20.

Persistent hashes, execution timestamps, platform labels and all check names are
in [public-release-evidence.json](public-release-evidence.json). The source verifier
was reviewed to ensure each check actually asserts the required behavior; its
report alone was not treated as proof. No read-path failures, checksum mismatches,
missing artifacts or untested advertised macOS architecture remain for this
public-read release contract.

### Shipped binary identities

| Architecture | Archive SHA-256 | Installed executable SHA-256 |
| --- | --- | --- |
| Apple Silicon | `a777bdaff2ed70eee70fd108d11c893c9d30c42ba781d697e977376721428826` | `7754c1833e8168126fff35eb3fc4477a5a9e60c493ec8489ce141571d16fa206` |
| Intel | `b0270d317ab857312383bcdccd506431e0078414a319892d57f7bece15b324e9` | `c26b04825f8569a5196b370010f6c9d87e37dccef976fc6ce338d24889857f3a` |

Limits remain explicit: future FxTwitter availability is not guaranteed;
authenticated search is still known to fail in the observed account, and no
claim is made here about authenticated timelines or complete reply trees.
