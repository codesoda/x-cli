# Implementation verification

## Authenticated retrieval flow — v0.8.1 preparation, 2026-09-12

Executed locally on macOS arm64 against the uncommitted
`test/authenticated-cache-flow` working tree. This closes the specific direct
integration-test gap recorded in [the original README audit](original-readme-audit.md),
not its broader completion requirements. That audit remains an unchanged
historical **v0.7.0** baseline, including its then-outstanding gaps.

The new private retrieval tests use real temporary account registration/config
resolution and cache files, synthetic credential loading, and an injected
read-only graph factory. They run the production orchestration path, including
Viewer verification before content, explicit-handle validation, operation/actor
mapping, pagination, cache writes and persisted rate limits. Evidence covers:

- Following and followers misses target only the verified actor; subsequent
  disk-cache hits still load/connect/query Viewer, with no content request.
- Mismatched Viewer fails with the static identity error before populated or
  deliberately corrupt content can be read. Expired Viewer, bootstrap failure,
  changed explicit handles and existing cooldowns also stop before content.
- Legacy wrong-shaped authenticated records refetch after matching Viewer.
  List metadata owners do not become the actor or cache scope for subsequent reads.
- A later-page rate limit preserves partial users, is not saved as a fresh
  collection, persists its cooldown and stops the next invocation before
  credentials; no retry or fallback occurs.
- Public reads and valid hits never invoke credentials/the graph factory even
  with an authenticated default, corrupt private content and private cooldown.
  Wrong-shaped public users/lists records are replaced by a synthetic Fx response.
- The production entry still calls the unchanged GraphQL constructor: an
  anonymous synthetic bootstrap response with the wrong hash fails before Viewer.
  No hash bypass or real authorization fixture was introduced.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --locked --offline app::retrieval::tests` | 10 direct flow tests passed |
| `cargo test --locked --offline` | 201 tests passed (180 lib, 6 CLI, 11 installer, 4 update integration) |
| `cargo test --locked --offline --features live-tests --test live` | 3 offline guards passed; 11 live cases ignored |

These tests substitute normalized graph results; unchanged provider tests cover
wire request definitions and parsing separately. Neither establishes live X
interoperability, private-list permissions or real Chrome/Keychain compatibility.
The preparation run used no live test, browser, Keychain or real credentials.
The primary then inspected the actual factory/test changes and reran the full
local gates successfully: stable locked build/test, formatting, all-target/
all-feature Clippy, warnings-denied rustdoc, Rust 1.88 locked build/test and
offline live guards, and Aislop with its existing configuration. GitHub CI and
published-install verification are recorded separately after they run; local
passes do not establish them. Historical results below are not fresh passes.
No mutation, policy/journal, credential-loading or state-durability changes are
included. Version/package/lock/README notes prepare v0.8.1, not evidence of publication.

## Self-update and dual-mode installer — verified 2026-09-11

Executed locally on macOS arm64 against the uncommitted working tree containing
the `xcli update` command and the rewritten dual-mode `install.sh`.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo build --locked` | Passed |
| `cargo test --locked` | 119 offline tests passed (98 lib, 6 cli, 11 installer, 4 update integration) |
| `cargo test --locked --all-features` | 121 offline tests passed; 3 live cases ignored |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps` | Passed |
| `cargo +1.88.0 test --locked` | 119 offline tests passed |
| `aislop ci` with gate 95 | Passed, exit 0; 0 errors, 0 warnings |
| `shellcheck install.sh` and `sh -n install.sh` | Passed |
| Repository visibility | Anonymous GitHub API reports `"private": false` (checked 2026-09-11) |

Deliberately not performed: no live `xcli update --check`/install against
GitHub releases (the published v0.1.1 predates the command; network seams are
offline-tested), no real release download or system-wide installation, no
browser/Keychain or authenticated X access, and no commit, push, tag or
release publication. Installer and update tests use injected child-process
tools and synthetic archives only.

## Phase 1 read-only CLI — verified 2026-09-10

Executed locally on macOS arm64, 2026-09-10. Stable compiler: Rust 1.95.0; declared MSRV additionally tested with Rust 1.88.0.

| Check | Result |
| --- | --- |
| `cargo build` | Passed |
| `cargo test` | 78 passed; feature-gated live target excluded |
| `cargo test --locked --all-features` | 80 offline tests passed; 3 live cases ignored |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps` | Passed |
| `cargo +1.88.0 build --locked` | Passed |
| `cargo +1.88.0 test --locked --all-features` | 80 offline tests passed; 3 live cases ignored |
| `actionlint .github/workflows/*.yml` | Passed |
| `aislop ci` with gate 95 | Passed, 100/100; no findings |
| Explicit FxTwitter public CLI test | Passed; all 16 checks in the public release verifier also passed on the source binary |
| Installer safety checks | Three offline tests covering curl/gh/auto installation and checksum/manifest/archive/version rejection passed; `shellcheck install.sh` passed |
| Explicit public X manifest/hash test | Passed, no authenticated query |
| Real Chrome/Keychain and authenticated GraphQL | User-reported post/parent/reply smoke stages passed; search HTTP 404 confirmed, header correction awaiting retry; timeline not reached. No browser/Keychain access by the agent |
| GitHub-hosted CI / release publishing | v0.1.1 release run 34540909288 passed every job, including native downloaded-install verification on Apple Silicon and Intel |
| Downloaded release works locally | Published v0.1.1 installer downloaded, binary installed into isolated directory, all 16 public checks passed; hashes matched GitHub assets. Latest/auto installation also passed. See public-release.md |

After splitting app orchestration and extracting cache/config/state/credential/provider tests, three additional app regression tests verify validation-before-access, local account dispatch and cooldown enforcement. All extracted test coverage is retained.

Tests cover strict IDs/URLs, backend/selector routing, account identity/ambiguity, concurrent config updates, credential redaction and consent, synthetic Chrome schema/encryption/domain/partition/expiry/Keychain denial, WAL consistency, physically isolated bounded caches and purge/permissions, rate limits/cooldowns, GraphQL wrappers/errors/schema drift, parent limits, repeated cursors, partial pages, and executable CLI behavior.

Live tests now reside in `tests/live.rs`, require the `live-tests` feature, and remain individually ignored with runtime opt-in/CI guards. `cargo test --all-features` runs the additional offline guard/redaction tests but never the three live cases. Source inspection, mock success and a public asset check do not establish authenticated interoperability. See [live verification](live-verification.md) for performed requests, defensible alternatives, blockers and the exact next user input/test sequence.

No Phase 2 operations, posting, DMs, automatic account/proxy rotation, or browser-session modification were implemented. Phase 2 was moved to [issue #1](https://github.com/codesoda/x-cli/issues/1).
