# Working on retrieval providers

Read [../../docs/protocol-research.md](../../docs/protocol-research.md) before
changing GraphQL definitions, feature values, headers or response assumptions.
Record new first-party source URLs/revisions/hashes and distinguish static
observations from consented live results.

## Protocol and privacy boundaries

- Allowlist read-query name, ID and operation type together. Never copy historical
  Bird fallback IDs blindly: a previously referenced search ID was observed as
  a mutation. No mutation operations or automatic alternate-account/provider
  fallbacks belong here.
- `operations.rs` owns the reviewed GraphQL snapshot. Keep variables, feature
  flags, toggles, response roots and pagination handling explicit and auditable.
  Do not guess transaction IDs or execute downloaded JavaScript.
- The public X asset is fetched without session headers and hash-verified before
  extracting its public authorization value. Never print that value or headers.
  Asset/query changes fail clearly rather than triggering an unsafe fallback.
- FxTwitter must never receive browser credentials or account-specific/protected
  content. Keep credential-bearing requests restricted to HTTPS X origins;
  preserve redirect rejection and the transport's request/response bounds.
- `Viewer` identifies the cookie owner. `UserByScreenName` does not. Preserve
  stable-ID pinning before authenticated cache/upstream reads in the app layer.
- Do not reuse upstream code without checking provenance/license and recording
  required attribution. Bird is a historical reference, not a runtime dependency.

## Parsing and pagination checklist

- [ ] IDs remain strings; malformed required fields fail with a protocol error.
- [ ] HTTP errors and GraphQL `errors` inside HTTP 200 are handled without printing
      raw bodies/messages. Distinguish auth, permissions, unavailable content,
      rate limits, network failure and drift where evidence permits.
- [ ] Missing roots/unknown instructions are not successful empty collections.
- [ ] Visibility wrappers, note text, unavailable posts and recognized module/
      replacement cursor instructions have synthetic parsing tests.
- [ ] Pagination is bounded, preserves opaque continuation cursors and deduplicates
      by stable ID. Repeated cursors and later-page failures remain explicit.
- [ ] Cursor exhaustion never establishes an exhaustive reply/search/timeline
      collection. Keep parent and reply completeness separately reportable.
- [ ] No request after a rate limit bypasses persisted cooldown checks. Do not add
      account/proxy rotation or silent retries as a compatibility workaround.
- [ ] Output includes backend/account provenance, local retrieval/cache metadata,
      and warnings for omitted or partial context—not invented certainty.

Run `cargo test providers::`, `cargo test pagination::`, the CLI integration tests,
and the root verification checks. Use injected transports and synthetic session
material only. Public-network tests are ignored by default; authenticated live
checks require separate consent and must not expose private payloads to agents.
