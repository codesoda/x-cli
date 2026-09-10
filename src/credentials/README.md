# Chrome credential boundary

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

## Practical limits / deviations

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
