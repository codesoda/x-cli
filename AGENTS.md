# Working on x-cli

Read [README.md](README.md) for actual supported behavior and
[docs/development.md](docs/development.md) for dependency and release conventions.
The initial README was a design, not evidence that a feature existed. Keep
implementation status and verification claims accurate.

## Documentation map

Use these references when the corresponding topic is relevant; historical
reports are evidence of what was done, not guarantees about current behavior.

| Document | Contents / when to consult |
| --- | --- |
| [README.md](README.md) | User-facing supported commands, installation, account semantics, security, exit codes and limitations. Read before changing CLI behavior; update it when behavior changes. |
| [CHANGELOG.md](CHANGELOG.md) | Notable changes and release notes. Update Unreleased for user-visible work; consult before preparing a versioned release. |
| [docs/development.md](docs/development.md) | Toolchain/MSRV, dependency rationale, verification commands, aislop settings and CI/release conventions. Consult for dependencies, tests, tooling or workflow changes. |
| [docs/implementation-plan.md](docs/implementation-plan.md) | Original Phase 1 sequence, assumptions, boundaries and initial public-API evidence. Consult for design intent or scope questions; check current code/README for actual status. |
| [docs/protocol-research.md](docs/protocol-research.md) | Pinned X/Bird/Chromium provenance, query definitions, storage details, attempted alternatives and unresolved protocol questions. Read before changing provider or credential assumptions; obtain fresh evidence when upstream changes. |
| [docs/live-verification.md](docs/live-verification.md) | Public checks performed, outstanding consent/profile requirements and a safe local authenticated test sequence. Read before proposing or performing live tests; the document itself is not consent. |
| [docs/verification.md](docs/verification.md) | Recorded implementation-check results and skipped verification. Consult when reporting prior evidence; rerun relevant checks rather than treating historical passes as current ones. |
| [docs/phase-2.md](docs/phase-2.md) | Preserved future list/bookmark/like/follow scope, proposed commands and mutation safeguards, linked to issue #1. Consult for roadmap boundaries, not authorization to add mutations to Phase 1. |

Keep this map current when adding, moving or removing `docs/*.md` files.
Directory-specific checklists live in
[src/credentials/AGENTS.md](src/credentials/AGENTS.md),
[src/providers/AGENTS.md](src/providers/AGENTS.md),
[tests/AGENTS.md](tests/AGENTS.md), and
[.github/workflows/AGENTS.md](.github/workflows/AGENTS.md).

## Scope and consent

- Phase 1 is read-only. No X mutations, posting, DMs, account/proxy rotation,
  browser-session switching or protection bypasses. Phase 2 is separately tracked
  in [issue #1](https://github.com/codesoda/x-cli/issues/1); do not implement it
  incidentally while fixing reads.
- Browser connection, Keychain access and authenticated live testing require
  explicit user consent. Never inspect real cookies or secrets through tools or
  put them in model context, logs, fixtures, errors or PRs.
- Source-verified GraphQL definitions and mocked tests do not prove live
  interoperability. Record live evidence separately and stop on unsupported
  storage, identity uncertainty, rate limits or protection denial.
- Do not push, create tags, or publish releases unless requested.

## Architecture and invariants

- `src/main.rs`: thin entry point, safe output and exit-code mapping.
- `src/cli.rs` / `src/app.rs`: argument definitions and top-level dispatch.
  `src/app/auth.rs` handles account commands; `task.rs` validates read tasks and
  cache keys; `retrieval.rs` handles capability-aware provider/cache orchestration.
  Keep incompatible flags as errors; never silently ignore an account selector.
- `src/model.rs`: normalized data, string IDs, provenance/freshness and explicit
  completeness. `src/error.rs`: reviewed static errors, never raw upstream bodies.
- `src/providers/`: isolated upstream definitions, transport use and parsing.
  Ordinary public reads use FxTwitter; explicit account selection implies X
  GraphQL. Never fall back from authenticated access to a public provider.
- `src/credentials.rs` and `src/credentials/`: session API and local credential
  access. Read [src/credentials/AGENTS.md](src/credentials/AGENTS.md) before editing
  either, including the sibling facade that nested instructions do not scope.
- `src/config.rs`: locally unique aliases, explicit `@handle` semantics, stable-ID
  identity pins. Preserve ambiguity checks for multiple connections; require a
  preference or explicit connection. Use transactional updates for CLI changes.
- `src/cache.rs` / `src/state.rs` (Unix implementation in `src/state/unix.rs`):
  authenticated scopes physically separate from
  public/other-account caches. Verify identity before authenticated cache hits.
  Preserve bounded storage, private permissions, atomic descriptor-relative
  writes, symlink/hardlink checks and persisted cooldowns. Purge content without
  clearing rate-limit state. Cached content is not encrypted at rest.
- `src/pagination.rs`: bound requests, detect repeated cursors, retain partial
  results on later failure, and never infer exhaustive collections from cursor
  exhaustion. Failed requests are not empty successes or fresh cache entries.

Keep modules focused and aim for fewer than 400 lines per file. Move substantial
inline test modules to `<module>/tests.rs` as private `#[cfg(test)]` child modules;
when production code is too large, split by responsibility instead of compressing
formatting or widening APIs just for tests.

## Verification checklist

Rust edition 2024, MSRV **1.88**. Keep `Cargo.lock` committed. Justify new
production dependencies; test-only helpers belong in dev-dependencies.

```sh
cargo build --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps
cargo +1.88.0 test --locked
aislop ci
```

- Normal tests are offline and synthetic. Never run ignored/live tests implicitly.
- Aislop gate is **95**, otherwise generated defaults. Never add ignore remarks
  unless explicitly instructed by the user. Do not lower thresholds to conceal defects.
- For workflow changes, run `actionlint .github/workflows/*.yml`.
- Update README/CHANGELOG and relevant protocol/security documentation when
  behavior changes. Report checks actually run, skipped live verification and
  remaining blockers; never claim GitHub CI passed from local tests alone.
