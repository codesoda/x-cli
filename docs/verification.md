# Implementation verification

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
