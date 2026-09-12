# Mutation activation contract and current blockers

Phase 2 development is authorized, but live X mutations are not. Production still
has only GET transport and no write commands. This document is a design/evidence
record, not permission to enable mutations or claim they work.

## Safety foundation before any operation is activated

- Write policy is disabled by default and keyed by stable account ID, not alias,
  mutable handle or connection. Read connection consent is not write consent.
  Future consent must name a reviewed operation set/revision; upgrading must not
  silently authorize newly added operations. Remove policy with the last account
  connection, but retain unresolved attempt records.
- Require explicit `--account`, normal connection ambiguity checks, operation
  policy, cooldown checks and a fresh matching Viewer identity before any write.
  Bind authorization to that verified session. Never fall back to another
  account/provider or accept a generic URL/body/method escape hatch.
- Separate typed mutation definitions/transport from the existing GET-only read
  manifest. Each needs exact source-backed name, ID, type, POST envelope,
  success/failure parser, source hash and reconciliation contract.
- Use a bounded, non-evictable per-account journal with at most one unresolved
  attempt per account initially. Persist a may-have-been-sent state **before**
  dispatch. Crash/restart must never replay it. Store only bounded IDs, operation
  revision, intended state and timestamps—not credentials or arbitrary bodies.
- An uncertain network/parse/outcome failure blocks later mutations until an
  explicit fresh same-account reconciliation resolves it. Missing items in
  bounded collections, missing fields, cursor exhaustion and generic 404s do not
  establish absence or no effect. Never automatically retry.
- Persist confirmed outcome before account-only cache invalidation, retaining an
  invalidation-pending state until cleanup succeeds. Preserve cooldowns/journal.
  The v0.9.1 global cache generation prevents pre-invalidation managed reads
  from refilling afterward, but does not implement this journal recovery or
  establish upstream freshness. Define journal/policy lock ordering before activation.
- Audit crash durability, including creation of parent directories, before
  claiming power-loss-safe journaling. Storage/capacity failures must block
  dispatch. Normal synthetic tests cover identity mismatch, policy, ambiguity,
  rate limits, errors, uncertain outcomes, crashes, concurrency, redaction,
  invalidation recovery and permission failures.

The first implementation can keep an empty production mutation allowlist and
exercise this state machine with injected transports only. A foundation release
must not advertise working add/remove/create/delete commands.

## Local durability audit — 2026-09-12

At v0.7.0, inspection of `src/state/unix.rs` confirmed that `write` synced the
temporary file and immediate parent after rename, but newly created ancestor
directories were not synced before descent. The v0.8.0 preparation adds syncs of
each newly created directory and its containing directory using pinned
descriptors, after private permissions are set. A sync failure stops that walk
before any descendant record is created. Synthetic tests check call ordering,
nested creation, injected sync failure, and non-creating read walks; existing
state security tests also pass.

The v0.8.2 hardening reprocesses **every opened directory link before descent**
on each creating walk, including existing directories left by a failed prior
invocation. There is no remembered already-synced state. Newly created pairs
unconditionally sync child then containing directory after private permissions
are set. For existing pairs, each descriptor is processed separately, child
first: fd-based `fstatvfs` must succeed; only `ST_RDONLY` exempts that existing
namespace descriptor, otherwise standard `File::sync_all` must succeed. A
readonly root never exempts a writable descendant. No sync error is swallowed,
and mount flags are not rechecked after a failure. File data, new pairs and
post-rename/unlink/prune syncs never receive readonly forgiveness.

Only successful `mkdirat` creation triggers intermediate-directory permission
repair; an `EEXIST` race is classified as existing and does not chmod that
ancestor. Final private-directory repair and descriptor-relative path, type,
owner and link protections remain unchanged. Non-creating read/removal/prune
walks do not perform traversal synchronization or create directories; their
existing post-unlink/prune syncs are unchanged. Walk failure prevents descendant
creation, record publication and lock actions, and a retry must process the
failed link again before proceeding.

This closes retry sequencing **conditionally**, assuming stable mount/path
topology, storage that honors synchronization, and independently durable exempt
readonly namespaces. `ST_RDONLY` is not persistence proof: readonly bind views,
volatile backing or error-remounted storage can violate that assumption. Extra
synchronization may surface storage errors on writable ancestors that were
previously unchecked. No permission, ownership or pathname heuristic establishes
readonly durability.

Synthetic canonical-temporary-directory tests check ordering, repeated failure
and retry, readonly child/parent combinations, mount-query and sync errors,
`EEXIST` races, permissions and publication/lock barriers. They do not simulate
physical power loss or lying storage hardware. This is **not** universal
power-loss-safe mutation journaling. Future journal activation still requires
eligible persistent storage, the policy/outcome protocol above and separate live
consent. No journal, POST capability or mutation activation has been added.

## Local account cache invalidation — v0.9.0, 2026-09-12

`cache purge --account <alias|@handle> [--connection <id>]` is local-only
maintenance. It resolves a registered stable ID from configuration and removes
only its GraphQL scope via `Cache::invalidate_scope`, under `.cache.lock` using
descriptor-relative `state::remove_file`. There is no scope enumeration or
content decoding; malformed/missing content does not require a provider request.
Symlink/hardlink entries are unlinked without following or chmodding their targets;
unexpected directories fail. Other scopes, cooldowns, config and any journal
records are not removed. Global purge is unchanged and never loads config.

The local resolver selects no browser profile and authorizes no session. Same-ID
profiles need no preference; handles reused across stable IDs remain ambiguous
unless an explicit connection agrees. No fresh Viewer identity is claimed, so
stale registered-account content can be cleared even if Chrome changed accounts.
Authenticated read/auth selection and identity checks are unchanged.

This is **local point-in-time deletion only**, not sufficient post-mutation
coordination. The cache lock serializes the unlink with cache writes, but a read
already in flight can subsequently refill stale content. Future mutation
activation needs account read/write serialization or cache generations, defined
lock ordering and journal `invalidation_pending` recovery after confirmed outcome
persistence. Synthetic tests cover isolation, malformed content, selector failures,
link safety and no external access—not this future coordination protocol.
No POST, write-policy enablement, journal, concurrency guarantee or mutation
activation is added, and the original-README audit baseline remains historical.

## Managed cache generation barrier — v0.9.1, 2026-09-12

This supersedes the v0.9.0 point-in-time-only coordination limitation above, not
its local-account resolution or privacy boundaries. One versioned
`cache/generation.json` record stores a global monotonically increasing u64;
missing metadata initially means zero. It uses only private state primitives,
with bounded constant-size integer metadata and no per-account map/lock files.
Corrupt/unsupported metadata and overflow are static Storage errors, never a
reset or a random/time-derived recovery guess.

Under the existing `.cache.lock`, scoped invalidation and global purge first
persist the increment, then perform their original content unlink/prune work.
Purge never deletes/resets the generation file. If deletion fails, generation
may conservatively have advanced. Global purge still ignores configuration;
scoped purge preserves unrelated content bytes, legacy mixed content, cooldowns,
config and any journal records. Both block before deletion on invalid generation
metadata or overflow.

Managed cache misses capture an opaque root-path-bound token before their first
upstream content request, after Viewer/explicit-handle checks on authenticated
reads. Capture releases the lock before fetch, including handle lookup, parent,
reply and post/user/list collection requests. Conditional save checks root and
generation and executes the original bounded/atomic per-scope put under that
same cache lock, without recursive locking. No account/cache lock is held across
network or credential access. A mismatch retains the successful/incomplete
collection with a static not-stored warning, not a request failure or retry.
Failed-request results stay uncached; refresh/TTL zero participate, no-cache
bypasses capture/writes, and ordinary cache hits are unchanged.

A global epoch is deliberately conservative and bounded: account-scoped purge
can suppress other scopes' in-flight writes, but must not delete or invalidate
those scopes' existing contents/hits. New reads after invalidation can refill.
The guarantee covers cooperating managed v0.9.1+ retrievals, not older binaries,
direct low-level `Cache::put`, manual metadata edits or unstable path topology.
It does not certify upstream freshness or authorize writes.

Deterministic synthetic temporary-state tests cover root binding, matching/stale
puts, scope/global invalidation, metadata corruption/overflow, security/size
guards and preserved state. Real retrieval-seam callbacks pause completion of
fake public/authenticated content while a local purge completes, then retain
returned data without stale storage; subsequent invocations cache successfully.
Post/parent/reply, user and list collections are exercised. This is not physical
power-loss evidence. Eligible persistent storage and the durability assumptions
above remain required, as do confirmed-outcome persistence, journal
`invalidation_pending` recovery, per-account policy and the reviewed protocol.
No journal, mutation transport, mutation policy activation or X write command is
implemented; the original-README audit remains an unchanged historical baseline.

## Bookmark source evidence — 2026-09-12

Anonymous static inspection used current
[main.ef8e0e0fdc2bb9c0a.js](https://abs.twimg.com/responsive-web/client-web/main.ef8e0e0fdc2bb9c0a.js),
SHA-256 `290a24f210c6b7310e3abe0abbf935a748e6b48719db6c9bf30782b44c206e59`,
and [LoggedInApiFilters.a9f8af9d64457a3aa.js](https://abs.twimg.com/responsive-web/client-web/bundle.LoggedInApiFilters.a9f8af9d64457a3aa.js),
SHA-256 `4605ee073ae503c1c034762d8db6dfcd61f49223d173cc04cb00ecbbf851754f`.
No JS execution, credentials, GraphQL calls, guest activation or mutations.

Main modules 84782/553263 define:

| Operation | ID | Type |
| --- | --- | --- |
| CreateBookmark | `aoDbu3RHznuiSkQ9aNM67Q` | mutation |
| DeleteBookmark | `Wlmlj2-xzyS1GN3a6cj-mQ` | mutation |

Both have empty feature/toggle metadata. Module 949428 sends `{tweet_id:id}`;
the adapter chooses POST with `content-type: application/json`, body
`{"variables":{"tweet_id":"<id>"},"queryId":"<operation-id>"}`, and no
features/fieldToggles properties. These definitions are **not** in xcli's runtime
allowlist.

Browser predicates check `tweet_bookmark_put === "Done"` and
`tweet_bookmark_delete === "Done"` after stripping `data`. The helper rejects
only when a wrong acknowledgement accompanies errors, so browser promise
resolution is not strict success evidence. Missing/wrong acknowledgements and
mixed errors must not be treated as confirmed application. Duplicate-add,
remove-absent, idempotency and timeout semantics remain unestablished.

## Activation blockers

1. **Exact-post reconciliation is not source-established.** The existing
   TweetResultByRestId manifest does not supply a field-selection document proving
   an explicit current-viewer bookmark boolean. Generic spreading of `legacy`
   and optimistic reducers assigning `bookmarked` are not authoritative reads.
   `data.tweetResult.result.legacy.bookmarked` is a candidate only; reliable false
   semantics, freshness and same-viewer meaning have not been established.
2. **Session binding is uncertain.** Both mutations appear in the session-binding
   allowlist. When enabled, filters sign method/path and add session signature,
   timestamp and token-hash headers; missing signing can be rejected. Public
   configuration sections conflict, so a cookie/CSRF envelope alone is not
   established as sufficient. No signing bypass, recovery retry or fabricated
   transaction ID is permitted.
3. **Live verification lacks consent and a named account/profile.** Separate
   consented testing is required, with sanitized outcomes only. Offline success
   cannot resolve server policy or authenticate a real account.

Needed evidence: first-party documentation/source tying an exact-post fresh read
to explicit true/false current-viewer bookmark state, a supported authentication
contract that respects session binding, and separately consented local evidence.
Until then bookmark write activation stays blocked. Other read-only work can
continue; this does not mark the overall original-README goal complete.
