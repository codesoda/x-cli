# Phase 2 — Account management

Phase 2 development was explicitly authorized on 2026-09-12. Live X mutations
were not authorized. The read-only increments are `bookmarks list` and own-account
`likes list`, plus list posts, members, known-list metadata and bounded
viewer-visible management inventory. Bookmark writes, recommendation discovery
and mutations, and like/unlike/follow/unfollow writes
remain future work. Own-account following/follower reads are available experimentally. Posting and DMs remain out of scope. Source verification and offline tests must not be presented as live
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
ranked/public-provider fallback or complete-list claim. List metadata is a separate
capability (below); discovery, create/update/delete and membership changes remain
unimplemented, not implied by timeline support.
Live list permissions and interoperability require separate consented evidence.

## Own-account relationship reads

Implemented experimentally in v0.5.0: `following list --account ...` and
`followers list --account ...`. Both use the selected account's verified Viewer
ID, require explicit account selection, and return `users:[{id,handle}]` plus
`posts:[]`. Post-result JSON remains compatible. User pages share bounded
pagination/failure handling and private cache isolation; a missing users array
in old cache data is never accepted as an empty relationship view. Neither
operation claims exhaustive enumeration or accepts arbitrary target users.
Source contracts are documented; live authenticated behavior remains unverified.

## List-member reads

Implemented experimentally in v0.6.0: `lists members list <list-id> --account ...`.
This is a separate users collection from list-post reads, keyed by list ID and
pagination in the verified-account cache scope. It requires explicit account
selection, validates list IDs, and exposes no membership-changing commands.
Source evidence and synthetic tests do not verify private-list permissions or
exhaustive membership.

## Known-list metadata

Implemented experimentally for v0.7.0: `lists show <list-id> --account ...`.
This non-paginated GET read requires an explicit account and canonical positive
list ID. Viewer identity verification precedes private cache access; the optional
metadata owner never selects the actor. Output is `posts:[]` plus one `lists`
record, with nullable description, visibility and owner, and no `users` field.
Unknown/missing visibility is not public by default. Missing/malformed metadata
fails closed without inferred deletion/private-denial claims. Completeness means
only one metadata record. Cache shapes are checked so legacy entries missing
`lists` cannot masquerade as success; account isolation and plaintext-at-rest
warnings apply. Discovery and mutations remain blocked; no live verification is
claimed.

## Viewer-visible list-management inventory

Implemented experimentally for v0.8.0: `lists list --account <alias|@handle>`.
This bounded GET query has no target user; Viewer verification precedes the
private cache. It returns incomplete `lists` output, not all owned/subscribed
lists. Observed `management_sections` can overlap; stable-ID deduplication unions
first-seen section provenance. Unsectioned rows need positive pinned/subscribed
flags or a known owner matching the verified actor. `is_member` is not ownership
or inventory evidence. Optional booleans preserve false vs unknown in inventory
and single-list metadata. Later observations fill missing values, but conflicts
keep the first known value with a static warning, never reconciliation certainty.

The reviewed non-Relay operation is separate from recommendations and from an
alternative Relay rollout with different aliases/owner projection. Missing or
unsupported shapes fail closed; no fallback or exhaustive absence claim is made.
Private metadata is not encrypted at rest. Existing bounds, cooldowns, partial
failure retention and `--no-cache` apply. No mutations or live availability are
established by source evidence and synthetic tests.

## Local account cache maintenance

Implemented for v0.9.0: `cache purge --account <alias|@handle>` with optional
agreeing `--connection` deletes only the registered stable account's GraphQL
content scope. This is local maintenance, not authenticated access: no credentials,
profile discovery, Viewer or provider call. Same-ID profiles need no preference
here; ambiguous reused handles still fail without disambiguation. Normal read/auth
resolution remains unchanged, and global purge still works without valid config.
Public/other-account scopes, registration metadata and cooldowns are preserved.

The primitive is only point-in-time deletion. In-flight reads may refill it;
future post-mutation cleanup still needs account read/write serialization or
generations and journal `invalidation_pending` recovery. No write policy, journal,
POST transport or mutation activation is included. The original-README audit
remains a historical baseline, not a claim that mutation safeguards are complete.

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

Implemented experimentally in v0.5.0 (synthetic coverage, not live-verified):

```sh
xcli following list --account @codesoda
xcli followers list --account @codesoda
```

Implemented experimentally in v0.6.0 (synthetic coverage, not live-verified):

```sh
xcli lists members list 123456789 --account @codesoda
```

Implemented experimentally for v0.7.0 (synthetic coverage, not live-verified):

```sh
xcli lists show 123456789 --account @codesoda
```

Implemented experimentally for v0.8.0 (synthetic coverage, not live-verified):

```sh
xcli lists list --account @codesoda --max-pages 2 --no-cache
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

## Mutation activation blockers

See [mutation-safety.md](mutation-safety.md) for the planned disabled-by-default
policy/journal/invalidation foundation and reviewed bookmark POST definitions.
Bookmark writes remain blocked on authoritative exact-post reconciliation and
supported session-binding requirements; no live mutation is authorized. Bounded
read collections cannot establish absence for retries.

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
