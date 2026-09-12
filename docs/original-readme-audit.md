# Original README progress audit (not completion)

Baseline: published and locally installed **v0.7.0**, 2026-09-12. The original
requirements are the design README preserved at commit `02c054a`, not later
feature advertisements. This snapshot records evidence and remaining gaps; it
must not be used to mark the continuing goal complete.

## Concrete goal deliverables

For each small increment: define bounded behavior, implement and test it on a
branch, push/open a PR, pass the applicable gates, merge, release, install using
the published installer, and retest the installed executable. Repeat on failure.
Complete all original functionality while retaining account/privacy safeguards,
and send ongoing progress DMs to Chris in cadence-app.

A successful release or public smoke suite is not proof that authenticated reads,
mutations, extra platforms, or every original requirement work.

## Prompt-to-artifact checklist

| Original requirement | Concrete artifact/evidence | Status at baseline |
| --- | --- | --- |
| Public post reads, JSON and human output | `src/providers/fx.rs`, `src/model.rs`, `src/app.rs`; `scripts/verify-public-release.py` asserts real public ID/URL/text/provenance | Implemented; installed public checks passed |
| Source backend, retrieval time, cache state, incomplete-context warnings | Normalized `Output`/`Provenance`, provider parsers and pagination; parsed installed public reports | Implemented, not upstream freshness guarantees |
| URL/ID validation and capability-aware backend routing | `src/input.rs`, `src/cli.rs`, `src/app/task.rs`, retrieval; offline and installed rejection checks | Implemented |
| Public reads default to FxTwitter; explicit account implies GraphQL, never fallback | `route`, retrieval branches and injected no-access tests | Implemented; authenticated endpoint availability remains separate |
| Browser profile discovery and explicit connection | `src/credentials/`, `auth add --consent`, config stable identity pin | macOS Chrome only; synthetic coverage plus historical user-reported connection/read stages |
| Aliases, @handle selectors, unique aliases, duplicate-account preference/ambiguity | `src/config.rs`, auth commands and routing tests | Implemented; no silent identity/profile switching |
| Secret minimization, supported OS access, no bypass/export | Credential modules/instructions and synthetic schema/encryption/redaction tests | Implemented constraints; broad real Chrome compatibility unverified |
| Private account caches, bounded storage, permissions, purge, cooldowns | `src/cache.rs`, `src/state/`; isolated synthetic scopes and public installed cache tests | Implemented; cached content is not encrypted |
| Parent chains, replies, pagination and explicit completeness | `src/pagination.rs`, GraphQL detail parsing, public parent-chain verifier | Public parents verified; user-reported authenticated parent/reply stages only |
| Authenticated search and timelines | Search/UserTweets definitions, request/parser tests, opt-in live smoke | Search had user-observed HTTP 404; corrected header not confirmed to resolve it; timeline stage was not reached |
| Additional browser/OS authentication after reads | Roadmap and credential platform guards | Not implemented; target scope and fresh storage/live evidence still needed |
| Lists: view | `lists posts` v0.4.0, `lists members list` v0.6.0, `lists show` v0.7.0; source hashes and synthetic tests | Implemented experimental views; inventory work continues; no live list-permission proof |
| Lists: create, update, delete, membership changes | Preserved commands/criteria in `docs/phase-2.md` | Not implemented; production remains GET-only |
| Bookmarks: list | `bookmarks list` v0.2.0; corrected optional toggles v0.5.1; request/parser/private-cache tests | Implemented experimentally; no live bookmark result |
| Bookmarks: add/remove, evaluate folders separately | `docs/mutation-safety.md` contains source-defined POST candidates and blockers | Not activated; exact-post reconciliation/session-binding unresolved; folders not implemented |
| Likes: list where supported | `likes list` v0.3.0, Viewer-derived target and History-rollout caveat | Implemented experimentally; live availability unverified |
| Like/unlike | Preserved roadmap and mutation safeguards | Not implemented |
| Following/followers | v0.5.0 own-account user collections, strict parser, private cache and pagination tests | Implemented experimentally; no exhaustive graph or live interoperability claim |
| Follow/unfollow | Preserved roadmap and mutation safeguards | Not implemented |
| Writes disabled by default with explicit per-account opt-in | No write commands/POST transport; policy/journal design in `docs/mutation-safety.md` | Absence of writes is not an implemented per-account write-policy system |
| Every write requires explicit account, verified actor, no fallback/retry | Design contract; read identity gate exists | Mutation gate and dispatch still unimplemented |
| Reconcile uncertain mutations before retry | Non-evictable journal/reconciliation design; bookmark evidence limitations | Not implemented; bounded collections cannot prove absence |
| Mutation failure/permission/identity/rate/ambiguity tests | Safety acceptance checklist and planned synthetic state machine | No activated mutation operation/test coverage yet |
| No posting, DMs, account/proxy rotation or browser-session switching | CLI/transport allowlists and instructions | Boundaries preserved |
| Branch, push, PR, passing CI, merge | PRs #4–#12 and GitHub check records | Per-increment evidence exists; not a functionality-completion proxy |
| Release and downloaded installation | v0.7.0 exact tag `9e658ae2f6e3facfae1aad18128ee5b403ac32cc`; release run 34679191830 | Both native macOS builds/download/install/public-verification jobs passed |
| Installed executable retest | v0.7.0 local 16 public checks + 7 metadata CLI guards, native report identity match; audit on PR #12 | Verified public/guard scope only, not authenticated metadata |
| Correct failures and rerun | Null-promotion parser fix before PR #9 merge; source-toggle correction in PR #10 | Fixes were gated before affected release; no live success inferred |
| Slack progress DMs | Slack CLI delivery acknowledgements to verified Chris DM in cadence-app | Ongoing; initial unavailable-skill gap was reported, not rewritten as delivered |

## Gate coverage and remaining verification gaps

The locally rerun gates include locked stable/MSRV builds/tests, formatting,
all-target/all-feature Clippy, warnings-denied rustdoc, and Aislop's unchanged
95 threshold. GitHub CI/release runs are checked independently against exact SHAs.
No workflow edits were required for these increments; historical actionlint
results are not claimed as a fresh check.

Normal tests use synthetic inputs and do not inspect real cookies or Keychain.
Installed public checks cover FxTwitter post/parent reads, output, routing and
cache controls. Additional CLI guards prove rejection behavior, not successful
live authenticated commands. Ignored authenticated tests remain separately
consented, profile/account-selected and unrun by the agent.

A focused review also identified a remaining integration-test gap: successful
Viewer → account-specific request/cache flow and mismatched Viewer before a
populated authenticated cache are not exercised end-to-end by the new app tests.
The code ordering was inspected, and session-failure/cooldown/cache-shape tests
exist; neither substitutes for that missing integration coverage.

Before dispatch-capable journaling, directory-chain crash durability must be
fixed and verified. Current file/immediate-parent syncs do not prove persistence
of newly created ancestor directories; see the local audit in
[mutation-safety.md](mutation-safety.md).

## Completion decision

**Not achieved.** Mutation functionality, write safeguards, extra-browser/OS
scope, consented authenticated interoperability and the identified verification
gaps remain. Continue defensible read/safety work, stop individual unsupported
operations on their documented blockers, and obtain necessary user/source
inputs without inspecting secrets or bypassing protections. Do not call the
thread goal complete based on release count, green gates, or this checklist.
