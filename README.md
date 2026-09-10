# x-cli

A planned read-first X/Twitter CLI with JSON output, interchangeable retrieval backends, and explicit multi-account support through local browser profiles.

## Status

Design stage. No CLI implementation yet. Command examples below describe the proposed interface.

## Proposed usage

```bash
xcli read <url-or-id>
xcli thread <url-or-id>
xcli search "query" --account personal
xcli user posts <handle>

xcli auth discover
xcli auth add --browser chrome --profile Default --alias personal
xcli auth add --browser chrome --profile "Profile 2" --alias work
xcli auth list
xcli auth default personal
xcli read <url> --account work
xcli doctor
```

## Architecture

Rust CLI → URL/ID parsing → cache → capability-aware backend router:

- **FxTwitter:** default for ordinary public-post reads; no user credentials sent to this service.
- **X GraphQL:** optional authenticated access using an explicitly connected local browser profile. Explicit account selection defaults to this backend.
- **Normalized output:** JSON by default, optional plain output, source backend, retrieval timestamp, cache status, and warnings about missing context or partial results.

Unofficial endpoints and third-party services can break or be restricted. This project does not promise uninterrupted access or complete reply trees.

## Multiple accounts and credentials

### Account selection

Use `--account <@handle|alias>` to select an account. A value starting with `@` resolves an X handle; any other value resolves a local alias. `--alias` is only for registering or renaming a connection, not selecting an account for a command.

```bash
# Discover the authenticated identity when connecting the profile
xcli auth add --browser chrome --profile Default --alias work

# Equivalent if work is connected to @codesoda
xcli read <url> --account @codesoda
xcli read <url> --account work
```

Aliases are optional, unique locally, and cannot start with `@`. Internally, connections are pinned to stable X account IDs rather than mutable handles. If multiple connections authenticate the same account, use its explicitly preferred connection or ask the user to choose; do not silently pick a different identity.

### Credential safeguards

- Start with macOS and Chrome; expand browser and OS support after verifying their credential-storage behavior.
- One browser profile per X account is the initial supported model. X's in-profile account switcher is not assumed to expose independent usable sessions.
- Profile discovery does not extract secrets. Connecting a profile requires explicit opt-in.
- Read only the X session material needed, using supported local OS credential facilities. Do not circumvent OS protections.
- Pin each connection to a verified X account ID. Refuse silent identity changes when the browser switches accounts.
- Store profile references and expected identities in configuration; load credentials locally on demand. Persistent secrets, if needed, belong in the OS credential store.
- Never expose session secrets to models, logs, normal command output, or FxTwitter.
- Browser sessions remain account-level credentials even when the CLI exposes only read operations.
- Isolate authenticated caches and rate-limit state by account; use restrictive permissions and provide cache purging.
- Respect rate limits and backoff. Never rotate accounts or proxies to evade restrictions.

## Roadmap

### Phase 1 — Read-only CLI

1. Public post reads through FxTwitter, normalized models, caching, and mocked tests.
2. Explicit browser-profile connection, identity validation, and authenticated GraphQL post reads.
3. Parent chains, replies, pagination, and explicit completeness reporting.
4. Search and timelines, then additional browser/OS support.

Keep GraphQL operation definitions and parsers isolated and test against sanitized fixtures. Verify current request behavior rather than assuming historical Bird endpoints still work.

### Phase 2 — Lists, bookmarks, likes, and follows

Planned authenticated account-management capabilities, subject to verification against current X GraphQL behavior:

- Lists: view, create, update, delete, and manage membership.
- Bookmarks: list, add, and remove.
- Likes/hearts: list where supported, like, and unlike posts.
- Follows: view following/followers, follow, and unfollow accounts.

These features use the selected account's authenticated GraphQL backend, not FxTwitter. Availability, pagination, and permissions must be verified per operation; bookmark folders and advanced list features require separate evaluation.

Proposed commands (not implemented):

```bash
xcli lists create "Builders" --account work
xcli lists members add <list-id> @someone --account work

xcli bookmarks list --account @codesoda
xcli bookmarks add <post-url> --account @codesoda
xcli bookmarks remove <post-url> --account @codesoda

xcli likes add <post-url> --account work
xcli likes remove <post-url> --account work

xcli following add @someone --account work
xcli following remove @someone --account work
```

Write safeguards:

- Writes are disabled by default and require explicit opt-in per account.
- Every mutation requires explicit `--account <@handle|alias>` and identity validation.
- Never fall back to another account or provider for a mutation.
- Do not blindly retry a mutation when its outcome is uncertain; report uncertainty and reconcile state where possible.
- Keep private data, including bookmarks, in account-isolated caches.
- Test mutation success, permission failures, rate limits, identity mismatches, and ambiguous network outcomes before enabling an operation.

## Boundaries

Phase 1 is read-only. Phase 2 adds only the account-management operations described above; posting and DMs remain out of scope. Automatic account switching, account pools, and proxy rotation are not planned. Verify Bird's source provenance and license before reusing any code; it is a protocol/design reference, not an assumed dependency.
