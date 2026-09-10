# Changelog

All notable changes to `xcli` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Experimental Phase 1 read-only Rust CLI work: public FxTwitter retrieval, normalized output, explicit account/profile configuration, account-isolated caching, bounded pagination, and isolated X GraphQL read-query definitions.
- Consent-gated macOS Chrome credential-loading work and stable-account identity safeguards, with synthetic credentials and mocked HTTP tests rather than live authenticated validation.
- Ubuntu and macOS CI for formatting, Clippy, locked builds/tests, rustdoc, and Rust 1.88 compatibility; tag-validated Apple silicon and Intel macOS release packaging with SHA-256 checksums and a mandatory CI gate.
- Best-README-Template-based README and contributor documentation covering installation, actual commands, exit codes, dependency rationale, credential-safe testing, and release conventions.
- Generated aislop project configuration and GitHub Actions quality gate, raised to 90; otherwise default settings, with no disabled rules.
- Per-scope bounded cache files, purge controls, persisted cooldowns, explicit local connection removal/renaming/preferences, and parent/reply completeness metadata.

### Known limitations

- Phase 1 is experimental. Authenticated X interoperability and real Chrome/Keychain compatibility remain unverified; source research and mocked tests do not establish that live authentication works.
- Unofficial endpoints may change or restrict access. Bounded thread/reply retrieval does not promise a complete conversation tree.
- Phase 2 lists, bookmarks, likes, and follows are tracked in [#1](https://github.com/codesoda/x-cli/issues/1) and [the Phase 2 plan](docs/phase-2.md), not implemented mutations. Posting and DMs remain out of scope.
