# Working on Chrome credentials

These instructions apply to this directory. When a change also touches the
public session API in `../credentials.rs`, read that file and preserve the same
invariants there; this nested AGENTS.md does not automatically scope its sibling.

Before changing storage or protocol assumptions, read
[`../../docs/protocol-research.md`](../../docs/protocol-research.md).
Live testing instructions are in
[`../../docs/live-verification.md`](../../docs/live-verification.md).
Never run browser/Keychain inspection commands to investigate a failure without
separate explicit user consent. Never put browser secrets in tool/model context.

## Required boundaries

Preserve the following behavior. Extending a supported schema, browser, or
platform requires current source evidence and synthetic tests—not a permissive
fallback when the current implementation rejects access.

- Production loading is macOS only. `Chrome::system` constructs the documented
  Stable root from absolute HOME without filesystem inspection. Explicit roots,
  metadata discovery, crypto and synthetic database tests are portable.
- Discovery lists immediate `Default` / canonical `Profile N` directory names
  only. No Local State/Preferences parsing, cookie access, profile labels or
  account inference. Numbers have at most ten ASCII digits, no leading zero
  except `0`. Symlink profiles are omitted.
- Consent is checked before every load and before any filesystem/Keychain work.
  Profile/database/Network/sidecar symlinks and special files are rejected;
  canonical paths must remain inside the selected profile. SQLite NOFOLLOW
  protects its main-file open. A trusted root may itself resolve through a
  symlink (including macOS's normal /var alias).
- Recognizes `profile/Cookies` and `profile/Network/Cookies` by metadata; exactly
  one must exist. No build/layout compatibility claim is made beyond synthetic
  tests. Both existing is ambiguous, never a reason to try one then the other.
- SQLite READ_ONLY + a single deferred transaction includes WAL state, with a
  bounded busy wait. No database copies, dumps, immutable mode, schema migration,
  process termination, permission changes, or authenticated network activity.
  SQLite can maintain its usual shared-memory WAL coordination; this is not a
  promise of zero filesystem metadata/SHM changes. Failures request closing
  Chrome, not disabling protection. Temporary SQLite work is memory-only.
- Supports **only version 24 and compatible-version 24**, real tables, encrypted
  `v10` only. Older/future schema, plaintext-only, mixed plaintext/encrypted,
  protected/unknown formats, bad padding, wrong exact-host SHA-256 prefix, empty
  key, invalid UTF-8/cookie bytes, or oversize values fail closed. No migration
  or plaintext/empty-key compatibility fallback.
- Only exact `.x.com` / `x.com`, `auth_token` / `ct0`, root-path, secure cookies
  with a secure source scheme and source port 443/unspecified are candidates.
  Persistent expiry must be strictly future Chromium-epoch microseconds;
  session cookies require zero expiry and both persistence flags off.
  Non-root/insecure/expired/other-host rows are ignored. This deliberately does
  not implement full browser cookie precedence or non-root cookie selection.
- Requires exactly one pair in the **same stored host jar**. No merging domain
  and host-only jars or selecting the newest duplicate. Any additional eligible
  row fails rather than guessing which independent values form a session.
  Partitioned candidates are rejected even if an unpartitioned pair exists.
  An empty top_frame_site_key means unpartitioned regardless of a valid 0/1
  ancestor bit: Chromium serializes a missing partition as `(empty, true)`.
  Source evidence: `net/cookies/cookie_partition_key.cc` (`Serialize` and
  `FromStorage`) at Chromium 233e625e16284f1f1e11150b88ea16e43c325c37.
- After the validated DB snapshot is closed, read only the existing generic
  password via `security_framework::passwords::get_generic_password` using
  service `Chrome Safe Storage`, account `Chrome`. All OS failures are static
  Permission errors. Respect prompts/denial; never create/reset an item or use
  a shell command, injection, alternate profile or protection bypass.
- PBKDF2-HMAC-SHA1, raw password bytes, saltysalt, 1003 rounds, 16-byte key;
  AES128-CBC, sixteen-space IV, strict PKCS7; verify raw SHA256(exact stored host)
  before stripping 32 bytes. Core is portable and tested with synthetic data.
- Session fields are private zeroizing strings with redacted Debug and no
  Serialize/Clone/Display. Headers contain only cookie auth_token/ct0 and the
  matching x-csrf-token; the GraphQL layer owns public authorization and must
  enforce HTTPS X-only routing and Viewer identity checks. The requested header
  API exposes zeroizing values, whose own Debug is NOT redacted: never log them.

## Limits that must remain explicit

Containment preflight plus NOFOLLOW is not a sandbox against another same-user
process concurrently replacing intermediate directories or SQLite sidecars.
SQLite opens these paths itself; atomic openat-style resolution of every file
would require a reviewed custom SQLite VFS. Normal browser writes are handled
by SQLite WAL locking/snapshots. Do not use an attacker-controlled profile root.
Hard-link provenance is likewise not established by canonical-path checks.

Owned password/key/decrypted buffers and session/header values are zeroized;
aes/cbc enable zeroization of cipher state. Rust/OS/SQLite/CFData/KDF internals,
allocator reallocation history, swap and HTTP client copies are not guaranteed
to be wiped. No claim of complete memory erasure is made. Encryption buffers
are bounded and secrets never enter dynamic error messages. Key lifetime is
only the single load; no persistent cookie/key cache exists.

Synthetic tests use only in-memory DBs and fresh temporary directories. They
cover consent order, denial injection, discovery, containment, both layouts,
schema rejection, digest/padding/format errors, ambiguity, partitioning, expiry,
minimal/redacted session API, read-only WAL visibility and coherent snapshots.
No test calls the production Keychain reader, `Chrome::system`, real browser
profiles or auth endpoints. Storage decoding does not prove live authentication.

## Change-review checklist

- [ ] Consent and profile validation precede filesystem or Keychain access.
- [ ] Discovery remains metadata-only and never infers a logged-in X identity.
- [ ] Cookie selection stays minimal, domain-coherent and unambiguous; unknown
      storage/protection fails closed without trying another profile.
- [ ] Tests cover the changed success path and rejection paths using only
      synthetic databases, keys and sessions. Include WAL/schema consistency,
      host-digest/padding, partition/expiry and OS-denial cases when relevant.
- [ ] No secrets can appear in Debug, errors, logs, fixtures or test failures.
      Zeroizing header values are still printable: never log them.
- [ ] No browser-session changes, permission changes, key creation/reset,
      database export, decryption bypass or public-provider credential exposure.
- [ ] The provider integration still verifies the live stable X identity before
      account-specific reads or authenticated cache access. Decryption is not
      identity verification.
- [ ] Update source evidence and documented limitations when assumptions change;
      distinguish synthetic validation from separately consented live results.

Run from the repository root:

```sh
cargo test credentials::
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps
aislop ci
```

The aislop CI gate is 95; do not lower it or weaken security checks to make a
change pass.
