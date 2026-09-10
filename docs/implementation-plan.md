# Phase 1 implementation plan

1. Build FxTwitter single-post read with injected HTTP transport, strict ID/URL validation, normalized JSON, safe structured errors and mocked tests.
2. Add private on-disk configuration/cache, stable-ID accounts, profile references, selectors and identity guards.
3. Implement explicitly consented macOS Chrome loading through OS Keychain, never exposing secrets or modifying browser sessions. Unsupported encryption fails closed.
4. Isolate current GraphQL definitions and parsing; add identity verification before reads, bounded parent/reply/search/timeline retrieval, cursor and completeness metadata.
5. Add diagnostics, CLI integration/security tests, documentation and discuss-cli-style CI/release workflows; run build/test/fmt/clippy/rustdoc and commit locally.

## Assumptions and boundaries
- No existing implementation; README at 02c054a is design only. No repository AGENTS.md exists; ancestor AGENTS.md is empty.
- Rust MSRV 1.88, edition 2024; lock dependencies and validate MSRV separately where available.
- JSON is default. No mutations, posting, DMs, fallback from authenticated to public, or browser-session manipulation.
- Normal tests use only temporary synthetic profiles, injected credentials and HTTP responses. No credential access or authenticated live requests in this implementation session without separate consent.
- Public sources and source code can verify request definitions, not successful authenticated behavior. Document this distinction. Unknown upstream changes must be errors, not empty results.
- Default public reads ignore the configured default account unless GraphQL is requested; an explicit account implies GraphQL. Search/timelines require GraphQL and resolve configured default if no selector.
- Bounded requests, no automatic retries (especially rate limits); expose retry advice and persisted cooldown. Thread/reply completeness is conservative.
- `ctx` prior-history lookup attempted but unavailable: generation verification failed / unsupported commit payload version 2. No prior history relied upon.
- Phase 2 moved to https://github.com/codesoda/x-cli/issues/1 and docs/phase-2.md.

## Public protocol evidence
FxEmbed official repository `FxEmbed/FxEmbed`, revision `9e71a25114b9d8d00c3381d4e797e94b357f001c`, `docs/specs/fxtwitter-openapi.json`: current v2 GET `/2/status/{id}`, SocialThread envelope; no credentials. Documentation landing page https://docs.fxtwitter.com/ . API details retrieved directly from pinned public source because old documentation paths returned the landing page. No upstream implementation code copied.
