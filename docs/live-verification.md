# Opt-in live verification

## Performed during implementation

On 2026-09-10, without browser credentials:

- FxTwitter `GET /2/status/20`: HTTP 200; normalized string ID `20`, author `jack`, text `just setting up my twttr`, known root.
- `cargo test --test cli live_public -- --ignored`: passed against FxTwitter.
- `cargo test --lib live_public_manifest -- --ignored`: passed. Downloads only the public X asset, verifies its pinned SHA-256, and finds the expected unique public web-client authorization value. It does **not** send cookies or execute Viewer/other GraphQL requests. The value is never printed.

No actual Chrome profile, Keychain item, or authenticated account was accessed. No authenticated interoperability claim follows from source inspection or synthetic tests. An interactive consent prompt could not complete in this harness; no consent was inferred.

## Remaining input needed

A user must explicitly approve local cookie/Keychain access and read-only X requests, and identify the Chrome Stable profile to connect. OS prompts must be approved directly by that user. Do not supply cookies, Safe Storage passwords, headers, or raw responses to an agent.

The safest next step is to run the following **yourself in a local terminal**, outside an agent transcript. Replace `Default` if needed and use an unused alias. Each command exposes only a redacted error (if any) and its exit code; successful post/account payloads are discarded rather than copied to chat.

```sh
cargo build --locked
./target/debug/xcli auth discover
./target/debug/xcli auth add --browser chrome --profile Default --alias smoke --consent >/dev/null
printf 'connection exit=%s\n' "$?"
```

Stop if connection fails. Do not attempt other accounts/profiles to evade a rate limit or protection. If successful:

```sh
./target/debug/xcli read 20 --account smoke --no-cache >/dev/null
printf 'post exit=%s\n' "$?"
./target/debug/xcli thread 20 --account smoke --replies --max-pages 1 --max-parents 1 --no-cache >/dev/null
printf 'conversation exit=%s\n' "$?"
./target/debug/xcli search 'from:jack' --account smoke --max-pages 1 --page-size 5 --no-cache >/dev/null
printf 'search exit=%s\n' "$?"
./target/debug/xcli user posts jack --account smoke --max-pages 1 --page-size 5 --no-cache >/dev/null
printf 'timeline exit=%s\n' "$?"
```

Run one at a time. Exit 12 is expected for bounded collections, but it can also report a later request failure; inspect the JSON locally if necessary, sharing only `complete`, `request_failed`, `stop_reason`, page count, and safe error kind—not content/cursors or raw responses. Stop on exit 6 and honor retry advice. Other errors need investigation before continuing, not blind retries. `--no-cache` avoids content caching, not rate-limit state.

Identity-change testing is a separate explicit manual step, not automated here: the user may change the account in that browser profile themselves, then rerun the original connection's read. It must fail with exit 10 and never return another account's cached content. The CLI must never switch Chrome on the user's behalf. Remove/re-add the registration explicitly to accept a new identity.

## Defensible alternatives investigated

See [Protocol research](protocol-research.md): current first-party Viewer and query manifest instead of stale Bird endpoints; native Keychain and schema-24 v10 decoding rather than shell exports or protection bypass; read-only SQLite transactions instead of inconsistent main-file copies; pinned source asset rather than executing remote JS or guessing transaction IDs. Historical REST identity fallbacks and Bird query-ID fallback lists are not silently attempted.

Real Chrome build compatibility, account-specific feature switches, transaction-header requirements, live Viewer semantics and additional pagination instruction variants remain unverified. On unsupported storage or X protocol rejection, provide the redacted error kind, command name, Chrome version, and macOS version; never the cookie database, credentials or raw authenticated response. A protected-key/OS denial is a stop condition, not permission for a workaround.
