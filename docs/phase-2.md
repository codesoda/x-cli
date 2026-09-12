# Phase 2 — Account management

Phase 2 development was explicitly authorized on 2026-09-12. Live X mutations
were not authorized. The read-only increments are `bookmarks list` and own-account
`likes list`, plus the latest-post list view `lists posts`. Bookmark writes,
list metadata/discovery and mutations, like/unlike writes, and follows remain
future work. Posting and DMs remain out of scope. Source verification and offline tests must not be presented as live
interoperability.

## First increment: bookmark reads

Acceptance criteria:
- `xcli bookmarks list --account <alias|@handle>`; a default account or
  `--connection` alone is insufficient for this private-data command.
- GraphQL only, with stable identity verified before private cache or upstream
  content access; no public or alternate-account fallback.
- Existing bounded `--max-pages`, `--page-size`, `--cursor`, cache controls and
  JSON/human output semantics; cursor exhaustion never implies completeness.
- Cache keys distinguish bookmark reads and pagination; content stays inside
  the existing backend/stable-account scope, with private permissions and purge
  controls. Content is not encrypted at rest; use `--no-cache` when appropriate.
- Source-reviewed GET query, synthetic request/parser/failure tests, and explicit
  user-run live verification kept separate. Never enable add/remove or folders
  incidentally while adding list support.

## Own liked-post reads

Implemented experimentally in v0.3.0: `likes list --account <alias|@handle>`.
The source-defined GET query is documented in protocol-research.md, including
its unverified History-rollout caveat. It uses the verified session owner's
stable ID, rejects arbitrary target users, and requires the same explicit
account and private cache boundaries as bookmarks. It exposes no like/unlike
writes, never infers exhaustive collections, and treats missing response roots or
wrong user discriminators as protocol errors. Source evidence and synthetic tests
cannot establish live availability; absent authoritative definitions are a stop
condition.

## Latest list-post reads

Implemented experimentally in v0.4.0:
`lists posts <list-id> --account <alias|@handle>` is a bounded read of posts in a
specific list. It requires an explicit account even for a public list, validates
the positive decimal list ID before external access, and scopes cache keys by
list ID, pagination and the existing verified-account boundary. There is no
ranked/public-provider fallback or complete-list claim. List discovery/metadata, create/update/delete, and
membership changes are separate capabilities, not implied by timeline support.
Live list permissions and interoperability require separate consented evidence.

## Scope
- [ ] Lists: view, create, update, delete, and manage membership.
- [ ] Bookmarks: list, add, remove; separately evaluate folders.
- [ ] Likes: list where supported, like, unlike.
- [ ] Follows: view following/followers, follow, unfollow.
- [ ] Verify current X GraphQL availability, pagination, permissions, and advanced list features per operation.

## Command status

Implemented experimentally in v0.2.0 (synthetic coverage; live interoperability
still unverified):

```sh
xcli bookmarks list --account @codesoda
```

Implemented experimentally in v0.3.0 (synthetic coverage, not live-verified):

```sh
xcli likes list --account @codesoda
```

Implemented experimentally in v0.4.0 (synthetic coverage, not live-verified):

```sh
xcli lists posts 123456789 --account @codesoda
```

Proposed commands below remain unimplemented:

```sh
xcli lists create "Builders" --account work
xcli lists members add <list-id> @someone --account work
xcli bookmarks add <post-url> --account @codesoda
xcli bookmarks remove <post-url> --account @codesoda
xcli likes add <post-url> --account work
xcli likes remove <post-url> --account work
xcli following add @someone --account work
xcli following remove @someone --account work
```

## Required safeguards / acceptance criteria
- [ ] Writes disabled by default; explicit opt-in per account.
- [ ] Every mutation requires explicit `--account <@handle|alias>` and identity validation against the stable pinned account ID.
- [ ] Never fall back to another account or provider for a mutation. FxTwitter receives no account-specific content or credentials.
- [ ] Do not blindly retry uncertain mutations; report uncertainty and reconcile state before permitting retries.
- [ ] Account-isolated caches for private data, including bookmarks; restrictive permissions and purge controls.
- [ ] Mock tests for success, permission failures, expired authentication, identity mismatches, rate limits, and ambiguous network outcomes before enabling writes.
- [ ] Live testing requires separate explicit consent, never credentials in logs or fixtures.
- [ ] No posting, DMs, automatic account switching, account pools, or proxy rotation.

Migrated from the design README at commit `02c054a` to keep the main README focused on actual supported functionality.
