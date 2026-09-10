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
xcli auth add personal --browser chrome --profile Default
xcli auth add work --browser chrome --profile "Profile 2"
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

## Implementation milestones

1. Public post reads through FxTwitter, normalized models, caching, and mocked tests.
2. Explicit browser-profile connection, identity validation, and authenticated GraphQL post reads.
3. Parent chains, replies, pagination, and explicit completeness reporting.
4. Search and timelines, then additional browser/OS support.

Keep GraphQL operation definitions and parsers isolated and test against sanitized fixtures. Verify current request behavior rather than assuming historical Bird endpoints still work.

## Boundaries

No posting, DMs, automatic account switching, account pools, or proxy rotation in the initial implementation. Verify Bird's source provenance and license before reusing any code; it is a protocol/design reference, not an assumed dependency.
