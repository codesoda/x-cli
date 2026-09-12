<a id="readme-top"></a>

<div align="center">
  <h1>x-cli</h1>
  <p>A read-only X/Twitter CLI with JSON output and explicit browser-profile accounts.</p>
  <a href="https://github.com/codesoda/x-cli/actions/workflows/ci.yml">CI</a> ·
  <a href="CHANGELOG.md">Changelog</a> ·
  <a href="https://github.com/codesoda/x-cli/issues">Report a bug / request a feature</a>
</div>

## Contents

- [About the project](#about-the-project)
- [Getting started](#getting-started)
- [Usage](#usage)
- [Accounts and browser setup](#accounts-and-browser-setup)
- [Output and exit codes](#output-and-exit-codes)
- [Security and caching](#security-and-caching)
- [Status and limitations](#status-and-limitations)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License and acknowledgments](#license-and-acknowledgments)

## About the project

`xcli` reads posts, parent chains, conversation views, searches, and user timelines. JSON is the default; `--human` renders posts for terminal reading. Post IDs remain strings.

**Phase 1 is experimental, not fully live-verified.** Public single-post retrieval has been exercised against FxTwitter. Authenticated operations use source-verified X web-client query definitions. A user-run live smoke test passed post, parent-chain and reply checks, then failed at search with exit 8; timeline testing was not reached. This is partial, user-reported verification—not a claim of full interoperability. Mock tests do not establish live authentication compatibility. See [status and limitations](#status-and-limitations).

### Built with

Rust (edition 2024, **MSRV 1.88**), clap, serde, reqwest/rustls, SQLite, and macOS Security.framework. Dependencies and developer checks are described in [Development](docs/development.md).

Modules separate CLI routing, normalized models, providers, credential loading, configuration, caching, pagination, and safe errors. FxTwitter is an optional third-party public source—not X, and not a recipient of browser credentials.

## Getting started

### Prerequisites

- macOS on Apple Silicon or Intel for prebuilt release binaries; no Rust installation required.
- Rust 1.88 or newer, Cargo, and a native compiler toolchain only when building from source.
- Internet access for upstream reads.
- **macOS + Chrome Stable** for browser-backed authentication. Other platforms can build the public CLI but cannot connect Chrome accounts.

### Installation

The repository is public. Install the latest
[GitHub release](https://github.com/codesoda/x-cli/releases) on macOS with an
anonymous download; no Rust toolchain or GitHub login is required:

```sh
curl -fsSL https://raw.githubusercontent.com/codesoda/x-cli/main/install.sh | sh
~/.local/bin/xcli --version
~/.local/bin/xcli read https://x.com/jack/status/20
```

The installer selects Apple Silicon/Intel, downloads over HTTPS only with bounded
timeouts, verifies the archive's SHA-256 checksum against the release manifest,
checks that the archive contains exactly the `xcli` binary reporting the expected
version, and installs atomically to `~/.local/bin/xcli` without sudo. Add
`~/.local/bin` to PATH. To inspect before executing, download `install.sh` first
and run it with `sh install.sh --release` (see `sh install.sh --help` for the
full mode and environment contract). Set `XCLI_VERSION=v0.5.0` to pin a release,
or `XCLI_INSTALL_DIR=/your/bin` to choose the destination. Downloads are
anonymous `curl` by default (`XCLI_DOWNLOAD_MODE=auto|curl`); set
`XCLI_DOWNLOAD_MODE=gh` to use an authenticated GitHub CLI instead. Archives and
`checksums-sha256.txt` are also available for manual installation.

Build from a local clone instead (requires Rust 1.88+):

```sh
git clone https://github.com/codesoda/x-cli.git
cd x-cli
./install.sh
```

Executed from an x-cli checkout, `install.sh` defaults to **source mode**: it
runs `cargo build --release --locked` with warnings denied, verifies the built
binary's version against `Cargo.toml`, and installs it atomically to the same
destination; a failed build leaves any existing installation unchanged. Pass
`--release` to download a prebuilt release from a checkout instead. A piped
script never source-builds from the working directory. `cargo install --path .
--locked` also works and places `xcli` in `~/.cargo/bin`.

The dual-mode installer and `update` command are available starting with
v0.1.2. Older binaries can be upgraded by rerunning the curl installer.

Release automation packages
both macOS architectures, then downloads the published installer/binaries and
runs explicit public read checks on each native platform. See
[public release verification](docs/public-release.md) for evidence and scope.

## Usage

```sh
# Public reads use FxTwitter, even if an authenticated default account exists.
xcli read https://x.com/jack/status/20
xcli read 20 --human

# Walk the parent chain, oldest first. This does not fetch replies.
xcli thread 20 --max-parents 20

# Explicit account selection implies authenticated GraphQL; no public fallback.
xcli read 20 --account work
xcli read 20 --account @codesoda
xcli read 20 --backend graphql

# Authenticated conversation view; not a complete reply tree.
xcli thread 20 --account work --replies --max-pages 3

# Authenticated search and timelines use the selected/default account.
xcli search "from:codesoda rust" --account work --max-pages 2 --page-size 20
xcli user posts codesoda --account work --max-pages 2

# Private bookmark reads require an explicit account; no add/remove commands.
xcli bookmarks list --account work --max-pages 2 --no-cache

# Only this verified account's own liked posts; no target-user or write commands.
xcli likes list --account work --max-pages 2 --no-cache

# Latest-post view of a known list; use its decimal list ID, not a post URL.
xcli lists posts 123456789 --account work --max-pages 2 --no-cache

# The selected account's own relationship views; output contains users, not posts.
xcli following list --account work --max-pages 2 --no-cache
xcli followers list --account work --max-pages 2 --no-cache

# Resume an upstream view using the returned opaque cursor.
xcli search "rust" --account work --cursor '<next_cursor>' --max-pages 1

xcli doctor
xcli cache purge

# Explicit self-update: check only, interactive confirm, or approve with -y.
xcli update --check
xcli update
xcli update -y
```

Accepted post inputs: positive decimal IDs; `x.com`/`twitter.com` status URLs, including `i/web/status` and photo/video suffixes. Unrelated hosts, malformed IDs and unsupported paths fail before requests.

### Routing and controls

| Option | Meaning |
| --- | --- |
| `--backend auto\|fxtwitter\|graphql` | `auto` uses public FxTwitter for ordinary post/parent reads; explicit account/connection, replies, search and timelines use GraphQL. |
| `--account @handle\|alias` | Select a registered account. Leading `@` means handle; otherwise local alias. |
| `--connection <id>` | Select a particular registered browser connection; must agree with `--account` if supplied. |
| `--max-parents <1..100>` | Parent-chain bound; default 20. Focal post is additional. |
| `--replies` | Thread only: retrieve GraphQL conversation pages in addition to the parent chain. |
| `--max-pages <1..20>` | Search/timeline/reply page bound; default 1. |
| `--page-size <1..100>` | Requested search/timeline page size; default 20. X controls actual response size. TweetDetail does not offer a verified count variable. |
| `--cursor <value>` | Resume the same operation/query/account; opaque, maximum 4096 bytes. |
| `--cache-ttl <0..86400>` | Maximum local cache age, seconds; default 300. Zero bypasses cache reads. |
| `--refresh` | Skip cache reads, replace result from upstream. |
| `--no-cache` | Skip cache reads and writes; cannot combine with `--refresh`. |
| `--data-dir <path>` | Override `~/.xcli` for configuration and cache. |
| `--human` | Human-readable posts instead of JSON (administrative commands remain formatted JSON). |

`bookmarks list` is available starting with v0.2.0, experimental and not live-verified. It requires an explicit
`--account` even when a default exists; `--connection` may disambiguate that
account but cannot replace the account selector. It uses the same text-only post
model, pagination bounds and conservative incomplete-collection output as search.
Bookmark folders and add/remove operations are not implemented. Cached bookmarks
are private account-scoped data but are **not encrypted at rest**; use `--no-cache`
to avoid content storage.

`likes list` is available starting with v0.3.0 and has the same explicit-account,
private-cache and conservative pagination requirements as bookmarks. It only
queries the Viewer-verified account's own liked posts: there is no target-user
argument or like/unlike command. It is experimental and **not live-verified**.
Some X rollouts redirect the browser to a newer History page; the source-defined
Likes query may be unavailable for those accounts. Rejection fails closed, with
no automatic History/other-user/provider fallback.

`lists posts <list-id>` is available starting with v0.4.0, experimental and
**not live-verified**. Supply a positive decimal list ID and explicit `--account`
(even for a public list); `--connection` may disambiguate that account but does not
replace it. The command reads the source-defined latest-post view, not ranked
results or an exhaustive archive. List discovery, metadata, create/update/delete
and membership commands are not implemented. It reuses the same pagination and
account-isolated cache controls as bookmarks; private-list content is not
encrypted at rest. Missing/inaccessible data fails without public fallback.

`following list` and `followers list` are available starting with v0.5.0 as
experimental, **not live-verified**, own-account relationship views. An explicit
`--account` is required; neither a default nor `--connection` alone suffices.
There is no target-user argument or follow/unfollow write command. Output adds
`users:[{id,handle}]` (including an empty array for a valid empty collection) and
has `posts:[]`; post reads retain their existing JSON without a `users` field.
Human output renders profile URLs. Pagination, private account-scoped cache
controls and incomplete-collection exit 12 apply; cached relationship data is
not encrypted at rest. Missing/unsupported user shapes fail explicitly, and no
result is claimed to be a complete follower/following graph.

`--backend fxtwitter --account work` and authenticated-only operations on FxTwitter are rejected. There is no automatic provider/account fallback. All requests are bounded by a 30-second timeout and an 8 MiB response limit. There are **no automatic retries**. Rate-limit responses persist a backend/account-scoped cooldown; honor the returned retry advice.

### Updating

`xcli update --check` reports the installed and latest released versions without changing anything. `xcli update` installs a strictly newer stable release only after an interactive terminal confirmation; `-y`/`--yes` skips the prompt and is required when stdin is not a terminal—without it the command fails before any network use. Updates never run automatically, never downgrade or reinstall an equal version, and only support the published native macOS artifacts (Apple Silicon and Intel); other platforms fail with exit 7—rebuild from source instead. The archive's SHA-256 is verified against the published checksum manifest **before** the candidate binary is ever executed; only the root-level `xcli` file is extracted, its `--version` is validated against the release, and the current executable (resolved through symlinks) is replaced atomically, remaining untouched on any failure. Update speaks only anonymous HTTPS to GitHub's release endpoints with bounded redirects, timeouts and download sizes, uses a transport separate from X reads carrying no session material, and never touches accounts, caches or credentials; `--data-dir` is rejected rather than ignored.

**`xcli update` requires v0.1.2 or newer.** For v0.1.1 and older, rerun the curl installer to upgrade.

## Accounts and browser setup

**Run connection commands yourself, locally. Never paste cookies, Keychain values, raw authenticated responses, or authorization headers into an agent/chat.**

```sh
# Directory discovery only: no cookie DB or Keychain reads.
xcli auth discover

# Explicit consent to X-session loading and a read-only live identity request.
xcli auth add --browser chrome --profile Default --alias personal --consent
xcli auth add --browser chrome --profile "Profile 2" --alias work --consent

xcli auth list
xcli auth default work
xcli auth prefer <connection-id>
xcli auth rename <connection-id> --alias office
xcli auth remove <connection-id> # local registration only; does not log out Chrome
```

Aliases are optional, locally unique, and cannot start with `@`. `--alias` is for registration/renaming only; it is not a read selector. Omit `--alias` on `auth rename` to clear it.

Connections store only a Chrome profile reference and the live-verified stable X account ID/handle. Each authenticated invocation loads the selected profile on demand and verifies the cookie owner's `Viewer` identity **before reading authenticated cached or upstream content**. A changed ID fails with `identity_mismatch`; the CLI does not silently reconnect, switch accounts, or modify Chrome. Handles in configuration are last-verified handles, not permanent IDs; remove the old registration and reconnect explicitly if an account changes its handle or the selected profile switches accounts.

Use separate Chrome profiles for different X accounts. X's in-profile account switcher is **not** assumed to supply independently usable sessions. If multiple profiles authenticate the same account, choose an explicitly preferred connection or pass `--connection`; an alias alone does not bypass this ambiguity check. A configured default selects an account, not permission to silently choose among duplicate connections.

Chrome may prompt for Keychain permission. Denial, unsupported encryption, unknown schema, inconsistent/ambiguous cookies, and identity uncertainty fail closed. Do not disable OS protections. Close Chrome manually if SQLite reports a lock; the CLI does not kill or alter it. Registration consent permits subsequent on-demand loading for this connection's read commands; session secrets are not persisted by xcli.

## Output and exit codes

Post/collection results contain:

- `posts`: normalized ID, author ID/handle, text, timestamp when supplied, parent ID/knowledge, canonical URL.
- `provenance`: backend, authenticated account ID or `null`, Unix-seconds retrieval timestamp, cache `hit`/`miss`, age in seconds.
- `complete`, `stop_reason`, `pages`, `next_cursor`, and `warnings`.
- `parent_chain_complete` / `replies_complete` when applicable, and `request_failed` when a partial result retained data after a failed request.

A single post can be complete **as a single post**, not as its surrounding context. Parent-only requests are complete only when a known root is reached. Search, timelines, and reply collections are always conservatively incomplete: cursor exhaustion does not prove X exposed every matching or protected/deleted post. A conversation view may include parent posts and other branches; it is not advertised as a filtered exhaustive direct-reply list.

Later page failures retain obtained posts, mark partial status, and emit a warning rather than claiming an empty success. First-request failure is an error. Request-failure results are not cached. No raw upstream response/error body is printed.

| Exit | Meaning |
| --- | --- |
| 0 | Successful single post, complete parent chain, or administrative operation |
| 2 | Invalid input/flags |
| 3 | Unavailable post/user; deletion only asserted when provider evidence says so |
| 4 | Permission denied |
| 5 | Expired/rejected authentication or missing connection consent |
| 6 | Rate limited / local cooldown |
| 7 | Unsupported operation/backend/platform/protection |
| 8 | Protocol changed or response not understood |
| 9 | Network/upstream service failure |
| 10 | Browser-session identity mismatch |
| 11 | Local state/configuration/cache error |
| 12 | Partial collection, with usable JSON on stdout |

Errors are JSON on stderr; results are JSON on stdout. GraphQL failures may include a `diagnostic` containing only a fixed stage label and numeric HTTP/upstream error code—never raw response strings or headers. Scripts must explicitly accept exit 12 when consuming bounded collections. Help/version are conventional text. IDs, content, and cursor values should always be treated as untrusted upstream data.

## Security and caching

- Phase 1 exposes **only GET read queries**. No posting, DMs, like/unlike mutations, follow/unfollow mutations, bookmark mutations, list mutations, account/proxy rotation, or browser-session switching.
- OS-supported, in-process Keychain retrieval; only necessary X cookies are selected. No shell-based secret output, persistent cookie export, or protection bypass.
- Only supported Chrome schema/encryption variants are decoded. Chromium source evidence and remaining compatibility questions are recorded in [Protocol research](docs/protocol-research.md).
- Secrets are excluded from serialization/debug/error output and practical owned buffers are zeroized. This is not a guarantee against memory inspection, OS swap, or a compromised machine.
- Browser credentials never go to FxTwitter or the public web-client asset host. Authenticated redirects are disabled; transport origins are allowlisted.
- GraphQL definitions are pinned to a reviewed first-party source snapshot. A public web-client asset is downloaded without cookies and hash-verified to obtain its public authorization value. No remote JavaScript is executed. Asset/query churn fails closed instead of trying historical IDs.
- Config/cache directories use Unix `0700` and files `0600`, atomic writes, and symlink checks. Caches are separated by backend and stable account ID, and from public caches. Cached content is **not encrypted at rest**; protect your user account/disk/backups. Storage is bounded to 64 backend/account scopes, each with at most 128 entries / 4 MiB; oldest entries/scopes are evicted. Custom state paths must not traverse symlinks (on macOS use `/private/tmp`, not `/tmp`).
- `xcli cache purge` removes all cached content, not connection configuration or rate-limit cooldowns. `--no-cache` avoids content caching but still honors rate limits. Remove the local config to revoke xcli's connection registrations; this does not revoke Chrome's session at X.
- `doctor` is deliberately local-only and does not inspect profiles, access Keychain, or test authentication.

## Status and limitations

| Area | Status |
| --- | --- |
| FxTwitter public post read | Implemented; public live read verified |
| Parent-chain traversal | Implemented; bounded, explicit missing-parent handling |
| macOS Chrome connection/Keychain | Synthetic coverage plus user-reported successful local connection/read stages; not accessed by the coding agent |
| X Viewer/post/replies/search/timeline | User-reported post/parent/reply smoke checks passed; search failed with exit 8, timeline not reached; see live-verification notes |
| Cache/account routing/security | Implemented; deterministic tests |
| Media, quotes, metrics, articles | Not normalized in this initial text-focused model; no promise of full rich-post fidelity |
| Self-update (`xcli update`) | Available starting with v0.1.2; offline-tested, explicit-only updates |
| Other browser/OS authentication | Unsupported |
| Bookmark listing | Experimental authenticated read; synthetic tests only, no live verification |
| Own liked-post listing | Experimental authenticated read; synthetic tests only, rollout availability unverified |
| List-post timelines | Experimental latest-post read; synthetic tests only, private-list permissions/live behavior unverified |
| Following/follower lists | Experimental own-account user collections; synthetic tests only, live behavior unverified |
| Phase 2 mutations | Unimplemented; [issue #1](https://github.com/codesoda/x-cli/issues/1) |

**Remaining live-verification blockers:** the user-observed search failure and untested timeline stage need further consented local evidence. Broader released-Chrome compatibility, X account-specific feature values, required transaction headers, identity semantics and pagination variants also remain unverified. The current client uses a documented public feature snapshot plus conservative optional-variable choices. It does not fabricate transaction IDs or circumvent browser/OS/network challenges. On rejection, capture only xcli's redacted error kind/exit code and operation name—not cookies or raw responses. See [protocol evidence and attempted alternatives](docs/protocol-research.md) and the [safe local live-verification sequence](docs/live-verification.md).

FxTwitter can disappear or change; X's unofficial web endpoints can change or restrict access without notice. There is no guarantee of uninterrupted operation, exhaustive search, complete replies, or source freshness. A fresh local retrieval timestamp does not certify that the third-party source is fresh.

## Roadmap

- [x] Read-only CLI, normalized text output, bounded pagination, local accounts/cache, deterministic tests.
- [ ] Complete consented authenticated interoperability testing and resolve any observed protocol/storage gaps.
- [ ] Phase 2: lists, bookmarks, likes and follows—tracked in [issue #1](https://github.com/codesoda/x-cli/issues/1), with [preserved requirements](docs/phase-2.md).

Posting and DMs remain outside the roadmap.

## Contributing

Open an issue or submit a focused pull request. See [Development](docs/development.md) and [Implementation plan](docs/implementation-plan.md). Aislop's generated default configuration is committed at `.aislop/config.yml`, with the CI gate raised to 95; default engines and telemetry settings remain unchanged. Run `aislop scan` or `aislop ci`; Rust checks remain mandatory alongside it. Normal tests must never inspect real browser profiles, use Keychain, or require credentials. Live integration tests live in `tests/live.rs`, gated by `--features live-tests`, `#[ignore]`, and explicit runtime opt-ins; they refuse CI execution. See the [local live-test guide](docs/live-verification.md). Live tests must be explicitly selected and read-only. Never attach secret-bearing fixtures or raw authenticated logs to issues.

Project contact: [codesoda/x-cli issues](https://github.com/codesoda/x-cli/issues).

## License and acknowledgments

No project license has been selected yet; do not infer a license from dependencies or reference projects. The package is not configured for crates.io publication.

- [Best-README-Template](https://github.com/othneildrew/Best-README-Template) for README organization.
- [FxEmbed](https://github.com/FxEmbed/FxEmbed) for the optional public API.
- [discuss-cli](https://github.com/codesoda/discuss-cli) for changelog and CI/release conventions.
- Peter Steinberger's Bird 0.8.0 as a historical protocol/design reference only. Its shipped MIT license and registry revision are recorded in [Protocol research](docs/protocol-research.md); no Bird implementation code is copied, installed, or given credentials.
- Chromium and X's public web-client source for current storage/query evidence, not a claim of official support.

<p align="right">(<a href="#readme-top">back to top</a>)</p>
