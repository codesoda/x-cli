# Changelog

All notable changes to `xcli` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.8.0] - 2026-09-12

### Added

- Experimental `lists list --account ...`: bounded viewer-visible management
  inventory using the source-reviewed `ListsManagementPageTimeline` GET query.
  Explicit account selection and Viewer-before-cache checks are required. No
  arbitrary target user, recommendations, Relay/public fallback or mutations.
- List placement provenance (`management_sections`) preserves overlapping pinned
  and owned/subscribed sections across duplicate rows/pages. Flat rows require
  explicit positive inventory evidence. Optional subscribed/pinned/member flags
  preserve unknown and false values, including in single-list metadata output.
  Duplicate conflicts keep first known values and emit a static warning; this is
  an incomplete visible snapshot, not exhaustive ownership or reconciliation.
- Synthetic request, strict parser, duplicate merge, pagination/failure/redaction,
  selector/cache isolation and legacy-output coverage, plus a separately ignored
  installed-binary smoke procedure. Authenticated inventory availability and the
  alternative Relay rollout remain **not live-verified**.

### Fixed

- Human output escapes ANSI/C1 and other terminal control characters (except
  layout newlines/tabs) instead of emitting provider-supplied control sequences.
  JSON content stays unchanged; use JSON for exact content/cursor values.
- Newly created state directories and their containing directories are synced
  before descent. Injected sync failures stop the current operation before
  descendant records are created. This is not a complete mutation-journal
  crash/recovery guarantee; remaining durability caveats are documented.

### Documentation

- Added an original-README requirement-to-artifact progress audit with explicit
  incomplete mutation/platform/live-verification requirements.

## [0.7.0] - 2026-09-12

### Added

- Experimental `lists show <list-id> --account ...` using the source-reviewed
  `ListByRestId` GET query. Requires explicit account selection and a canonical
  positive decimal list ID; verifies Viewer before account-scoped cache access.
  No paging, discovery, public fallback or mutation transport is enabled.
- Single-record `lists` metadata output with string ID, name, optional description,
  visibility and owner, plus canonical list URL and human rendering. Existing
  post/user JSON stays compatible. Missing visibility is never assumed public;
  malformed metadata and mismatched IDs fail closed. Completeness applies only
  to the single metadata record, never to list membership or posts.
- Synthetic exact-request, parser, redaction, routing, cache-shape/isolation and
  output compatibility tests, plus a separately ignored installed-binary metadata
  smoke procedure. Source evidence does not establish live private-list
  permissions or authenticated interoperability; metadata remains not live-verified.

## [0.6.0] - 2026-09-12

### Added

- Experimental `lists members list <list-id> --account ...` with a source-reviewed
  GET query, explicit account and validated list ID. Returns a bounded users
  collection using private account-scoped cache keys distinct from list-post
  reads. No membership mutations or public fallback; no exhaustive membership or
  live permission/interoperability claim.
- Synthetic request/root/pagination/error and private routing/cache tests plus a
  separately ignored installed-binary list-member smoke procedure.

## [0.5.1] - 2026-09-12

### Fixed

- Omit `fieldToggles` on Bookmarks and ListLatestTweetsTimeline requests when
  their source callers supply no toggle options. Direct adapter reinspection
  corrected an earlier empty-object assumption; exact-request tests now check
  omission. This aligns a source contract, not a proven live failure cause.

### Documentation

- Record disabled-by-default mutation safety design and bookmark activation
  blockers: authoritative exact-post reconciliation and session-binding behavior
  remain unestablished. No write commands or POST transport are enabled.

## [0.5.0] - 2026-09-12

### Added

- Experimental own-account `following list --account ...` and
  `followers list --account ...`, with source-reviewed GET queries, explicit
  account selection, Viewer-derived IDs, bounded pagination and private scoped
  caches. No arbitrary target users, follow/unfollow writes or public fallback.
  Live availability, permissions and complete pagination remain unverified.
- Normalized `users:[{id,handle}]` for relationship collections, profile-link
  human output, and shared post/user pagination with stable-ID deduplication and
  partial-failure retention. Post JSON remains compatible; relationship results
  include `posts:[]`. Old cache entries lacking the required users array are
  bypassed rather than treated as empty success.
- Synthetic model/cache, pagination, user parser, request/routing and failure
  tests, plus separately ignored installed-binary live procedures. No live
  authenticated relationship checks were performed by the agent.

## [0.4.0] - 2026-09-12

### Added

- Experimental `lists posts <list-id> --account <alias|@handle>` using the
  source-reviewed latest-list GET query. Requires an explicit account and
  validated decimal list ID; retains identity-before-cache checks, bounded
  pagination, private account-scoped caches and conservative completeness.
  No ranked/public fallback, discovery/metadata or list/membership writes.
- Synthetic list-post request, module/replacement cursor, repeated cursor,
  empty/partial results, redaction, input/routing and cache-guard tests; a separate
  opt-in installed-binary live smoke procedure. Actual list permissions and
  authenticated interoperability remain unverified.

### Changed

- Split GraphQL response parsing from HTTP/query construction without changing
  existing normalization or public parser APIs.

## [0.3.0] - 2026-09-12

### Added

- Experimental `likes list --account <alias|@handle>` for the verified account's
  own liked posts, using a source-reviewed GET query. Explicit account selection,
  stable identity verification, private cache scopes, bounded pagination and
  conservative completeness are preserved. No arbitrary target users, like/unlike
  writes, or automatic History/provider fallbacks. Source-defined availability
  is not live interoperability; newer History rollouts may reject this query.
- Synthetic own-likes request, parsing, failure/redaction, routing and cache-guard
  tests, plus a separately opt-in installed-binary private smoke procedure.
- Independent source-pinned bookmark feature-name fixture to detect shared
  metadata drift; corrected Phase 2 docs to distinguish implemented reads from
  unimplemented writes.

## [0.2.0] - 2026-09-12

### Added

- Experimental read-only `bookmarks list --account <alias|@handle>` with a
  source-reviewed GET query, bounded pagination, JSON/human output and private
  account-isolated caching. Requires an explicit account, verifies its stable
  identity before cache/upstream content access, and never uses a public fallback
  or claims exhaustive completeness. Bookmark add/remove and folders are not
  implemented. Synthetic tests and a separately opt-in installed-binary smoke
  procedure are provided; authenticated bookmark interoperability is unverified.

## [0.1.3] - 2026-09-12

### Added

- Opt-in live CLI smoke tests accept `XCLI_LIVE_BINARY` to verify a trusted
  installed executable instead of the Cargo build, preserving explicit consent,
  profile validation, CI refusal, and redacted failure output. This adds a
  verification path, not evidence of authenticated interoperability.

## [0.1.2] - 2026-09-11

### Added

- Explicit `xcli update` self-update command with `--check` and `-y`/`--yes`.
  `--check` is non-mutating; installation requires an interactive terminal
  confirmation or `--yes` (a non-terminal stdin without `--yes` fails before
  any network use). The release archive's SHA-256 is verified against the
  published manifest before the candidate is ever executed, only the
  root-level `xcli` archive member is extracted, the candidate's `--version`
  is validated, and the current executable is replaced atomically with every
  failure path preserving the existing binary. Equal or older releases are
  never installed; only native macOS Apple Silicon/Intel release artifacts
  are supported. Update uses a bounded anonymous HTTPS transport separate
  from X reads and never initializes accounts, caches or credentials;
  `--data-dir` and read-command flags are rejected rather than ignored. The
  command is available starting with v0.1.2.
- Source mode for `install.sh`: executing the script from an x-cli checkout
  (validated by the `Cargo.toml` package name beside it) builds with
  `cargo build --release --locked` and warnings denied, verifies the built
  binary's version, and installs it atomically; build failures leave any
  existing installation unchanged. `--source`/`--release` select a mode
  explicitly, `--help` documents the contract, and a piped script can never
  source-build from the working directory. Offline installer tests now cover
  both modes with injected `uname`/`curl`/`gh`/`cargo` stubs.
- New production dependencies `flate2` and `tar` for in-process release
  archive extraction during self-update; see the dependency rationale in
  [docs/development.md](docs/development.md).

### Changed

- The repository is now public. The installer defaults to anonymous `curl`
  downloads (`XCLI_DOWNLOAD_MODE=auto` resolves to `curl`); `gh` remains
  available when selected explicitly. `XCLI_VERSION` and `XCLI_DOWNLOAD_MODE`
  now fail loudly in source mode instead of being silently ignored.
- Documentation describes the anonymous public installation flow. The v0.1.0
  anonymous-404 history and the v0.1.1 private-repository audit evidence are
  preserved as historical records in
  [docs/public-release.md](docs/public-release.md).

## [0.1.1] - 2026-09-10

### Fixed

- Support authenticated GitHub CLI downloads for private repositories in the installer and native post-release verification. Anonymous URLs for this private repository return 404; repository visibility is unchanged.
- Test explicit GitHub CLI and auto-selection download modes with injected offline tools.

## [0.1.0] - 2026-09-10

First public-read release. Authenticated functionality remains experimental; search is known to fail in the observed account and is not part of the verified public-read claim.

### Added

- Checksum-verifying macOS release installer, with isolated offline installer tests and explicit post-publication download/install/public-read verification on Apple Silicon and Intel.
- Repeatable public release verifier covering ID/URL reads, JSON/human output, real multi-post parent chains and limits, cache controls, routing rejection and local diagnostics.
- Safe GraphQL failure diagnostics distinguish HTTP status, numeric upstream error codes, response roots and parser stages. Live tests forward only this typed metadata, never raw stderr or upstream messages.

- Local live integration-test target behind the `live-tests` feature, with ignored public checks and a sequential authenticated read smoke test requiring separate consent and an existing account/profile. Runtime CI/opt-in guards keep normal tests offline.

- Scoped AGENTS.md contributor checklists for credentials, providers, tests and workflows, plus a root architecture/documentation map; credential guidance moved from its module README.

- Experimental Phase 1 read-only Rust CLI work: public FxTwitter retrieval, normalized output, explicit account/profile configuration, account-isolated caching, bounded pagination, and isolated X GraphQL read-query definitions.
- Consent-gated macOS Chrome credential-loading work and stable-account identity safeguards, with synthetic credentials and mocked HTTP tests rather than live authenticated validation.
- Ubuntu and macOS CI for formatting, Clippy, locked builds/tests, rustdoc, and Rust 1.88 compatibility; tag-validated Apple silicon and Intel macOS release packaging with SHA-256 checksums and a mandatory CI gate.
- Best-README-Template-based README and contributor documentation covering installation, actual commands, exit codes, dependency rationale, credential-safe testing, and release conventions.
- Generated aislop project configuration and GitHub Actions quality gate, raised to 95.
- Per-scope bounded cache files, purge controls, persisted cooldowns, explicit local connection removal/renaming/preferences, and parent/reply completeness metadata.

### Changed

- Split app dispatch, account commands, read-task validation and retrieval into focused modules; separate Unix state storage from its public facade.
- Extract substantial credential, GraphQL, cache, config and state unit tests into private child-module files, preserving coverage and adding app dispatch/cooldown regression tests.

### Fixed

- Align GraphQL GET requests with the reviewed X adapter's `Content-Type: application/json` header. The user-observed search HTTP 404 still needs a consented retry; no endpoint-ID or method fallback was introduced.

### Known limitations

- Phase 1 is experimental. A user-run authenticated smoke test passed post, parent-chain and reply stages, then failed at search with exit 8; timeline was not reached. Full interoperability and broader Chrome/Keychain compatibility remain unverified.
- Unofficial endpoints may change or restrict access. Bounded thread/reply retrieval does not promise a complete conversation tree.
- Phase 2 lists, bookmarks, likes, and follows are tracked in [#1](https://github.com/codesoda/x-cli/issues/1) and [the Phase 2 plan](docs/phase-2.md), not implemented mutations. Posting and DMs remain out of scope.
