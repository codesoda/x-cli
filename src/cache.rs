//! Cached normalized read results and persisted per-backend/account cooldowns.
//! Keys are digests, not user input paths; credentials are never accepted here.
use crate::{
    error::{Error, Kind, Result, storage},
    model::{Output, now},
    state,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

mod generation;
pub use generation::{CacheGeneration, ConditionalPut};

type Entries = BTreeMap<String, Output>;
type Cooldowns = BTreeMap<String, u64>;

// Each scope is independently readable and stays well below state's 16 MiB cap.
// Normal completed writes occupy at most 256 MiB, plus cooldown/lock metadata.
const MAX_SCOPE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SCOPE_ENTRIES: usize = 128;
const MAX_SCOPES: usize = 64;
const MAX_COOLDOWNS: usize = 4096;

fn encoded_size(value: &impl serde::Serialize) -> Result<usize> {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len())
                .ok_or_else(|| std::io::Error::other("JSON size overflow"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_err(|_| storage())?;
    Ok(counter.0)
}

fn trim(entries: &mut Entries, keep: &str) -> Result<()> {
    let mut bytes = encoded_size(entries)?;
    let mut eviction = entries
        .iter()
        .filter(|(key, _)| key.as_str() != keep)
        .map(|(key, value)| {
            Ok((
                value.provenance.retrieved_at,
                key.clone(),
                encoded_size(key)? + 1 + encoded_size(value)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    eviction.sort();
    for (_, key, size) in eviction {
        if entries.len() <= MAX_SCOPE_ENTRIES && bytes <= MAX_SCOPE_BYTES {
            break;
        }
        entries.remove(&key);
        bytes -= size + usize::from(!entries.is_empty());
    }
    if entries.len() > MAX_SCOPE_ENTRIES || bytes > MAX_SCOPE_BYTES {
        return Err(storage());
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

fn digest(backend: &str, account: Option<&str>, key: Option<&str>) -> Result<String> {
    // JSON encodes tuple boundaries and distinguishes the public None scope from
    // every account string (including an empty string).
    let bytes = serde_json::to_vec(&(backend, account, key)).map_err(|_| storage())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn matches_scope(output: &Output, backend: &str, account: Option<&str>) -> bool {
    output.provenance.backend == backend && output.provenance.account_id.as_deref() == account
}
impl Cache {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn scope_path(&self, backend: &str, account: Option<&str>) -> Result<PathBuf> {
        Ok(self
            .root
            .join("content")
            .join(format!("{}.json", digest(backend, account, None)?)))
    }

    pub fn get(
        &self,
        backend: &str,
        account: Option<&str>,
        key: &str,
        ttl: u64,
    ) -> Result<Option<Output>> {
        // Resolve exactly one scope. Public reads never open or deserialize an
        // authenticated scope, an index, or the old mixed cache.json file.
        let entries: Entries =
            state::read_json(&self.scope_path(backend, account)?)?.unwrap_or_default();
        let Some(mut output) = entries.get(&digest(backend, account, Some(key))?).cloned() else {
            return Ok(None);
        };
        if !matches_scope(&output, backend, account) {
            return Err(storage());
        }
        let Some(age) = now().checked_sub(output.provenance.retrieved_at) else {
            return Ok(None);
        };
        // TTL=0 disables hits; expiry is exclusive, including its exact boundary.
        if age >= ttl {
            return Ok(None);
        }
        output.provenance.cache = "hit".into();
        output.provenance.age_seconds = age;
        Ok(Some(output))
    }
    /// Uncoordinated low-level write: callers can refill after invalidation.
    /// Managed retrievals must capture a generation before fetching and use
    /// `put_if_generation` instead.
    pub fn put(
        &self,
        backend: &str,
        account: Option<&str>,
        key: &str,
        output: &Output,
    ) -> Result<()> {
        state::with_lock(&self.root.join(".cache.lock"), || {
            self.put_locked(backend, account, key, output)
        })
    }

    // Caller owns .cache.lock. Never acquire that lock recursively here.
    fn put_locked(
        &self,
        backend: &str,
        account: Option<&str>,
        key: &str,
        output: &Output,
    ) -> Result<()> {
        if !matches_scope(output, backend, account) || output.provenance.retrieved_at > now() {
            return Err(storage());
        }
        let key = digest(backend, account, Some(key))?;
        // Count without allocating a second potentially huge serialized output.
        // Oversized outputs are not cached; remove any old hit for this key.
        let fits = encoded_size(output)? + encoded_size(&key)? + 3 <= MAX_SCOPE_BYTES;
        let path = self.scope_path(backend, account)?;
        let mut entries: Entries = state::read_json(&path)?.unwrap_or_default();
        entries.remove(&key);
        if fits {
            entries.insert(key.clone(), output.clone());
        }
        trim(&mut entries, &key)?;
        state::write_json(&path, &entries)?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(storage)?;
        // Enumerate metadata only: no other account's content is opened.
        state::prune_files(&self.root.join("content"), Some(filename), MAX_SCOPES)
    }
    /// Advance the global generation before deleting only this scope, without
    /// decoding it. Other content and cooldowns survive, but all older managed
    /// fetches lose write eligibility. A failed deletion may advance generation.
    /// This does not replace future post-mutation journal recovery.
    pub fn invalidate_scope(&self, backend: &str, account: Option<&str>) -> Result<()> {
        state::with_lock(&self.root.join(".cache.lock"), || {
            self.advance_generation_locked()?;
            state::remove_file(&self.scope_path(backend, account)?)
        })
    }
    /// Advance generation, then remove cached content only. Cooldowns and the
    /// generation metadata survive; a failed deletion may advance generation.
    pub fn purge(&self) -> Result<()> {
        state::with_lock(&self.root.join(".cache.lock"), || {
            self.advance_generation_locked()?;
            state::prune_files(&self.root.join("content"), None, 0)?;
            // Pre-isolation caches are never read or migrated; purge removes the
            // obsolete mixed file too, without following a possible symlink.
            state::remove_file(&self.root.join("cache.json"))
        })
    }
    pub fn check_cooldown(&self, backend: &str, account: Option<&str>) -> Result<()> {
        let entries: Cooldowns =
            state::read_json(&self.root.join("cooldowns.json"))?.unwrap_or_default();
        let deadline = entries
            .get(&digest(backend, account, None)?)
            .copied()
            .unwrap_or(0);
        let remaining = deadline.saturating_sub(now());
        if remaining > 0 {
            let mut error = Error::new(
                Kind::RateLimit,
                "Backend/account is cooling down; retry later",
            );
            error.retry_after_seconds = Some(remaining);
            return Err(error);
        }
        Ok(())
    }
    pub fn cooldown(&self, backend: &str, account: Option<&str>, seconds: u64) -> Result<()> {
        let key = digest(backend, account, None)?;
        state::with_lock(&self.root.join(".cache.lock"), || {
            let path = self.root.join("cooldowns.json");
            let mut entries: Cooldowns = state::read_json(&path)?.unwrap_or_default();
            let current = now();
            entries.retain(|_, deadline| *deadline > current);
            // Never evict an active rate limit to make room for another. Fail
            // closed at a bounded metadata size instead.
            if !entries.contains_key(&key) && entries.len() >= MAX_COOLDOWNS {
                return Err(storage());
            }
            let deadline = current.saturating_add(seconds);
            let old = entries.entry(key).or_default();
            *old = (*old).max(deadline);
            state::write_json(&path, &entries)
        })
    }
}

#[cfg(test)]
mod tests;
