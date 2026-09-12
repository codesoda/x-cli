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
  Serialize same-account reads/writes or use generations to prevent stale reads
  from refilling a just-invalidated cache. Define lock ordering before activation.
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
