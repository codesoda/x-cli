# Working on tests

Keep normal tests offline, deterministic, and safe on a developer's machine.
The same rules apply when adding inline unit tests in `src/`.

## Test checklist

- [ ] Inject `Transport` and `CredentialProvider`; do not read real Chrome
      profiles, call the production Keychain reader, or require X credentials.
- [ ] Use synthetic identities, cookie values, databases and response fixtures.
      Never capture raw authenticated responses or real cookie databases for
      fixtures. A test failure must not dump secret-bearing request headers.
- [ ] Create temporary state/profile roots. Canonicalize temporary paths where
      secure storage rejects symlink ancestors (notably macOS `/var`/`/tmp`).
      Do not weaken production containment checks to accommodate a fixture.
- [ ] Check JSON structure, string IDs, provenance, cache status, completeness,
      stop reasons and exit codes—not only success or snapshot appearance.
- [ ] Assert incompatible routing and missing consent perform no credential/
      network access. Cover identity mismatch, duplicate-account ambiguity,
      cross-account/public cache isolation, redaction and cooldown preservation.
- [ ] Exercise failures and boundaries: HTTP 200 GraphQL errors, unknown schemas,
      unavailable posts, repeated/exhausted cursors, page limits and later-page
      failures. Exit 12 has usable partial JSON, not necessarily a clean fetch.
- [ ] Keep global environment changes out of parallel tests; inject paths/state.

## Live tests are separate

Keep network tests explicitly ignored/opt-in. Never add `--ignored` to normal CI
or treat permission to run the offline suite as permission to access a browser.
Authenticated testing needs explicit consent and a named user-controlled profile;
follow [../docs/live-verification.md](../docs/live-verification.md). Do not switch
or modify a browser session to manufacture an identity-mismatch test.

Passing mocks or a public-asset check does not establish authenticated behavior.
Report what was actually exercised, and retain live-verification gaps in docs.

Run `cargo test --locked`, `cargo +1.88.0 test --locked`, and root formatting,
Clippy/rustdoc/aislop checks after changes. An individual test command is useful
while iterating, but is not evidence that the full suite passed.
