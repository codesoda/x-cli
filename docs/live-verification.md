# Local, opt-in live integration tests

`tests/live.rs` is a separate Cargo integration-test target gated by the
**`live-tests`** feature. Every network/browser test also has `#[ignore]` and a
runtime opt-in guard. Normal `cargo test`, even `cargo test --all-features`, does
not perform live work. The feature-enabled target has ordinary offline tests
for consent/CI guards, diagnostic redaction and executable selection.

The live tests refuse execution when common CI markers are present (`CI`,
`GITHUB_ACTIONS`, `GITLAB_CI`, `TF_BUILD`, `JENKINS_URL`, `BUILDKITE`). Do not add
`--ignored` or live opt-in variables to CI. These safeguards prevent accidental
execution; they are not an OS sandbox or a replacement for explicit consent.

## Public checks

Run yourself locally:

```sh
XCLI_LIVE=1 cargo test --locked --features live-tests --test live public_ -- --ignored --test-threads=1
```

- `public_post_and_parent_chain`: invokes the actual CLI against FxTwitter for
  post `20` and its known root/parent chain. No browser access.
- `public_manifest`: downloads only the pinned, credential-free X asset, verifies
  its SHA-256 and finds the expected public web-client authorization value. A
  test transport rejects any session headers or other URL before network access.
  No GraphQL identity/post request is made and the value is never printed.

CLI checks use `--no-cache`. The state directory defaults to `~/.xcli`; optional
`XCLI_LIVE_DATA_DIR` selects a different directory. Cooldowns persist there even
when a test fails. Nothing automatically purges cooldowns or retries a failure.

## Authenticated checks: separate explicit consent

**The coding agent has not accessed a real Chrome profile, Keychain item or
authenticated account.** A user-run smoke test has since provided partial live
evidence (see below). Source inspection and mocks do not establish live
interoperability. Adding the feature is not permission for an agent to run it.

First, connect the intended Chrome Stable profile yourself using the ordinary
consented flow. Do not paste cookies, Keychain values, headers or raw responses
into an agent/chat. For example, with an unused alias:

```sh
cargo build --locked
./target/debug/xcli auth discover
./target/debug/xcli auth add --browser chrome --profile Default --alias work --consent >/dev/null
```

Stop if connection fails. Do not try other accounts/profiles to evade a rate
limit or protection. Once connected, the following explicit invocation consents
to on-demand local browser-session loading and read-only X requests:

```sh
XCLI_LIVE=1 XCLI_LIVE_AUTH=1 \
XCLI_LIVE_ACCOUNT=work XCLI_LIVE_PROFILE=Default \
cargo test --locked --features live-tests --test live authenticated_read_smoke -- --ignored --test-threads=1
```

By default CLI smoke tests invoke the Cargo-built executable. To verify a
trusted installed release instead, set `XCLI_LIVE_BINARY` to its absolute path:

```sh
XCLI_LIVE=1 XCLI_LIVE_AUTH=1 \
XCLI_LIVE_ACCOUNT=work XCLI_LIVE_PROFILE=Default \
XCLI_LIVE_BINARY="$HOME/.local/bin/xcli" \
cargo test --locked --features live-tests --test live authenticated_read_smoke -- --ignored --test-threads=1
```

Check the installed executable's `--version` first and record it with the test
result. The override must name an existing absolute file; empty, relative,
missing, or directory paths fail rather than falling back to the Cargo binary.
Only select a trusted binary: it will receive the local data-directory path and
account selector. All consent/CI/profile guards still apply. The override also
applies to `public_post_and_parent_chain`, but not `public_manifest`, which
exercises the source library rather than an executable. Setting the override
alone never enables live tests. No installed authenticated run is claimed here.

The test requires both account and profile names, resolves the existing local
connection, and verifies that it matches the named profile **before loading
credentials**. Use `XCLI_LIVE_CONNECTION=connection-N` when necessary to explicitly
select among duplicate connections; otherwise normal preference/ambiguity rules
apply. `XCLI_LIVE_DATA_DIR` must match the directory used when registering the
connection. The test does not create, remove, rename or change connections or
modify/switch Chrome's session.

`authenticated_read_smoke` is one sequential test covering:

1. Viewer-verified post `20` retrieval.
2. Parent-chain retrieval, bounded to one parent.
3. Conversation/reply retrieval, at most two pages.
4. Search for `from:jack`, at most two pages with requested page size five.
5. User timeline for `jack`, with the same pagination bound.

Each CLI invocation verifies the pinned live identity before its read. All
returned account/post payloads are captured locally and never printed. Tests
check provenance, request-failure state, pagination bounds and completeness;
assertions do not dump expected/actual identities or content. Failures report
only a stage, safe assertion message, exit code or strictly typed protocol
diagnostic (fixed stage labels and numeric HTTP/upstream error codes). Raw
stderr/messages are never forwarded. No secret environment
variables or cookie arguments are accepted by these tests.

The sequence stops immediately on failure, including rate limits. Exit 12 is
accepted for bounded collections **only when `request_failed` is false**. Data
is not cached, but rate-limit cooldowns remain in the existing state directory
across runs. No account/proxy rotation, parallel account testing, automatic
retries or protected-content fallback is performed.

Do not use `--show-output`, instrument raw HTTP dumps or attach captured
responses to issues. If a test fails, share only the test/stage name, redacted
error kind/exit code and Chrome/macOS versions. Inspect further output only
locally, never by sending credentials/private payloads to a model.

## Separate private bookmark smoke test

`authenticated_bookmark_smoke` is separately ignored and never added to the
existing read/search/timeline smoke sequence. It requires the same explicit
local consent, registered account/profile, identity checks and CI refusal. It
requests one page of five bookmarks, accepts a valid empty collection, requires
conservative incomplete output, and withholds all captured private post data.
No content cache is written; existing cooldowns are preserved.

After installing v0.2.0 or newer with `bookmarks list`, run yourself locally only if
you consent to that private read (replace the account/profile as appropriate):

```sh
XCLI_LIVE=1 XCLI_LIVE_AUTH=1 \
XCLI_LIVE_ACCOUNT=work XCLI_LIVE_PROFILE=Default \
XCLI_LIVE_BINARY="$HOME/.local/bin/xcli" \
cargo test --locked --features live-tests --test live authenticated_bookmark_smoke -- --ignored --test-threads=1
```

This is a procedure, not recorded live evidence. The agent has not run it.
Do not share raw bookmark content, cookies, headers, or response bodies.

## Separate own-liked-post smoke test

`authenticated_own_likes_smoke` is separately ignored and not added to either
existing authenticated sequence. It uses the same account/profile/CI/consent
checks and private-data withholding as the bookmark smoke, but requests one
page of the selected account's own liked posts. No arbitrary target user is
accepted. After installing v0.3.0 or newer, a consenting user may run:

```sh
XCLI_LIVE=1 XCLI_LIVE_AUTH=1 \
XCLI_LIVE_ACCOUNT=work XCLI_LIVE_PROFILE=Default \
XCLI_LIVE_BINARY="$HOME/.local/bin/xcli" \
cargo test --locked --features live-tests --test live authenticated_own_likes_smoke -- --ignored --test-threads=1
```

The agent has not run this test. The source-defined Likes endpoint may be
unavailable on accounts routed to X's newer History UI; stop on rejection or
rate limits rather than trying another account or endpoint. Share only the fixed
stage, exit code and typed diagnostic, never private liked-post content.

## Verification evidence

On 2026-09-10, the original public checks passed before moving into this
feature-gated target:

- FxTwitter `GET /2/status/20`: HTTP 200; normalized string ID `20`, author `jack`,
  expected public text and known root.
- The pinned public X asset/hash and unique authorization-value extraction check
  passed without authenticated requests.

The target's compilation and offline opt-in/redaction guards are tested. A user
reported an authenticated run that passed the post, parent-chain and reply
stages, then failed at **search, exit 8**. The timeline stage was not reached.
This is user-reported partial success on one profile, not independently observed
agent testing or broad platform compatibility.

The original test withheld all stderr, so the search failure did not distinguish
HTTP rejection, GraphQL errors, a missing response root or a parser mismatch.
Fixed-label diagnostics now expose that distinction without exposing payloads.
The user's isolated retry confirmed **HTTP 404**, before response parsing. Public
source reinspection found the same query ID and GET method, but showed that
xcli omitted the adapter's `Content-Type: application/json` header. That header
is now sent and covered by a mocked search-request test. It is not yet known
whether the correction resolves the user's 404. No parser rules, account
boundaries, operation IDs or method allowlists have been relaxed.

To isolate search without repeating the passed stages, run locally:

```sh
cargo run --locked -- search 'from:jack' --account work --max-pages 1 --page-size 5 --no-cache >/dev/null
```

Replace `work` with the connected alias or `@handle`. Share only the resulting
structured error/diagnostic, never raw HTTP bodies or
cookies. Honor a rate-limit error before rerunning. Alternatively rerun the
explicitly consented smoke test; it now reports the typed diagnostic on failure.

## Remaining evidence and defensible alternatives

See [Protocol research](protocol-research.md): current first-party Viewer and
query manifest instead of stale Bird endpoints; native Keychain/schema-24 v10
rather than shell exports or protection bypass; read-only SQLite transactions
instead of inconsistent main-file copies; pinned public assets rather than
executing remote JS or guessing transaction IDs. Historical REST identity
fallbacks and Bird query-ID fallback lists are not silently attempted.

Real Chrome build compatibility, account-specific feature switches,
transaction-header requirements, live Viewer semantics and additional
pagination variants remain open until consented tests provide evidence. A
protected-key/OS denial is a stop condition, not permission for a workaround.

Identity-change testing is a separate explicit manual step: the user may change
the account in their browser themselves, then rerun the original connection's
read. It must fail with exit 10 rather than return another account's content.
The CLI/tests must not switch Chrome to manufacture this case. Accepting a new
identity requires explicit removal/reconnection by the user.
