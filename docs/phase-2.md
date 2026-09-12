# Phase 2 — Account management

Phase 2 development was explicitly authorized on 2026-09-12. Live X mutations
were not authorized. The first increment is read-only `bookmarks list`; bookmark
writes, lists, likes, and follows remain future work. Posting and DMs remain out
of scope. Source verification and offline tests must not be presented as live
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

## Scope
- [ ] Lists: view, create, update, delete, and manage membership.
- [ ] Bookmarks: list, add, remove; separately evaluate folders.
- [ ] Likes: list where supported, like, unlike.
- [ ] Follows: view following/followers, follow, unfollow.
- [ ] Verify current X GraphQL availability, pagination, permissions, and advanced list features per operation.

## Proposed commands (not implemented)
```sh
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
