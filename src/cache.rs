//! Cached public post data and persisted per-backend/account cooldowns.
//! Keys are digests, not user input paths; credentials are never accepted here.
use crate::{
    error::{Error, Kind, Result, storage},
    model::{Output, now},
    state,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

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
    pub fn put(
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
        // Oversized outputs are deliberately not cached (and replace no old hit).
        let fits = encoded_size(output)? + encoded_size(&key)? + 3 <= MAX_SCOPE_BYTES;
        state::with_lock(&self.root.join(".cache.lock"), || {
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
        })
    }
    /// Remove cached content only. Cooldowns and their deadlines survive a purge.
    pub fn purge(&self) -> Result<()> {
        state::with_lock(&self.root.join(".cache.lock"), || {
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
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Cache) {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().canonicalize().unwrap().join("state"));
        (dir, cache)
    }
    #[test]
    fn isolated_backend_stable_account_public_scope_and_key() {
        let (_dir, cache) = fixture();
        let output = Output::new("one", Some("123".into()));
        cache
            .put("one", Some("123"), "../../query", &output)
            .unwrap();
        for (backend, account, key) in [
            ("two", Some("123"), "../../query"),
            ("one", Some("456"), "../../query"),
            ("one", None, "../../query"),
            ("one", Some("123"), "different"),
        ] {
            assert!(cache.get(backend, account, key, 100).unwrap().is_none());
        }
        let hit = cache
            .get("one", Some("123"), "../../query", 100)
            .unwrap()
            .unwrap();
        assert_eq!(hit.provenance.cache, "hit");
        assert_eq!(hit.provenance.account_id.as_deref(), Some("123"));
        cache
            .put("one", None, "k", &Output::new("one", None))
            .unwrap();
        assert!(cache.get("one", None, "k", 100).unwrap().is_some());
        assert!(cache.get("one", Some(""), "k", 100).unwrap().is_none());
        assert_ne!(
            digest("ab", Some("c"), Some("d")).unwrap(),
            digest("a", Some("bc"), Some("d")).unwrap()
        );
        assert!(
            !std::fs::read_to_string(cache.scope_path("one", Some("123")).unwrap())
                .unwrap()
                .contains("../../query")
        );
    }
    #[test]
    fn physically_separated_corrupt_account_does_not_affect_public() {
        let (_dir, cache) = fixture();
        let mut private = Output::new("one", Some("123".into()));
        private.stop_reason = "private-account-marker".into();
        cache.put("one", Some("123"), "k", &private).unwrap();
        cache
            .put("one", None, "k", &Output::new("one", None))
            .unwrap();
        cache
            .put("two", None, "k", &Output::new("two", None))
            .unwrap();
        let public_path = cache.scope_path("one", None).unwrap();
        let account_path = cache.scope_path("one", Some("123")).unwrap();
        assert_ne!(public_path, account_path);
        assert_ne!(public_path, cache.scope_path("two", None).unwrap());
        assert_eq!(
            std::fs::read_dir(cache.root.join("content"))
                .unwrap()
                .count(),
            3
        );
        assert!(
            !std::fs::read_to_string(&public_path)
                .unwrap()
                .contains("private-account-marker")
        );
        std::fs::write(&account_path, b"corrupt authenticated JSON").unwrap();
        // The obsolete mixed cache is ignored too, even if it is malformed.
        std::fs::write(
            cache.root.join("cache.json"),
            b"corrupt legacy mixed content",
        )
        .unwrap();
        assert!(cache.get("one", None, "k", 100).unwrap().is_some());
        assert!(cache.get("two", None, "k", 100).unwrap().is_some());
        assert!(cache.get("one", Some("123"), "k", 100).is_err());
        cache
            .put("one", None, "k", &Output::new("one", None))
            .unwrap();
        assert_eq!(
            std::fs::read(&account_path).unwrap(),
            b"corrupt authenticated JSON"
        );
        cache.purge().unwrap();
        assert_eq!(
            std::fs::read_dir(cache.root.join("content"))
                .unwrap()
                .count(),
            0
        );
        assert!(!cache.root.join("cache.json").exists());
    }
    #[test]
    fn bounded_entries_bytes_and_oversized_outputs_remain_recoverable() {
        let (_dir, cache) = fixture();
        const {
            assert!(MAX_SCOPE_BYTES < state::MAX_JSON_BYTES);
        }
        let path = cache.scope_path("one", None).unwrap();
        let mut entries = Entries::new();
        for n in 0..MAX_SCOPE_ENTRIES {
            let mut output = Output::new("one", None);
            output.provenance.retrieved_at = now() - (MAX_SCOPE_ENTRIES - n) as u64;
            entries.insert(digest("one", None, Some(&n.to_string())).unwrap(), output);
        }
        state::write_json(&path, &entries).unwrap();
        cache
            .put("one", None, "latest", &Output::new("one", None))
            .unwrap();
        let entries: Entries = state::read_json(&path).unwrap().unwrap();
        assert_eq!(entries.len(), MAX_SCOPE_ENTRIES);
        assert!(!entries.contains_key(&digest("one", None, Some("0")).unwrap()));
        assert!(cache.get("one", None, "latest", 100).unwrap().is_some());

        let mut older = Output::new("one", None);
        older.provenance.retrieved_at = now() - 2;
        older.warnings.push("a".repeat(MAX_SCOPE_BYTES / 3));
        let mut newer = older.clone();
        newer.provenance.retrieved_at += 1;
        let entries = Entries::from([
            (digest("one", None, Some("older")).unwrap(), older),
            (digest("one", None, Some("newer")).unwrap(), newer),
        ]);
        state::write_json(&path, &entries).unwrap();
        let mut output = Output::new("one", None);
        output.warnings.push("b".repeat(MAX_SCOPE_BYTES / 2));
        cache.put("one", None, "latest", &output).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() <= MAX_SCOPE_BYTES as u64);
        assert!(cache.get("one", None, "older", 100).unwrap().is_none());
        assert!(cache.get("one", None, "latest", 100).unwrap().is_some());
        output.warnings = vec!["x".repeat(MAX_SCOPE_BYTES)];
        cache.put("one", None, "latest", &output).unwrap();
        assert!(cache.get("one", None, "latest", 100).unwrap().is_none());
        cache
            .put("one", None, "latest", &Output::new("one", None))
            .unwrap();
        assert!(cache.get("one", None, "latest", 100).unwrap().is_some());
    }
    #[test]
    fn scope_eviction_is_bounded_and_never_reads_other_scope_content() {
        let (_dir, cache) = fixture();
        for n in 0..MAX_SCOPES {
            let path = cache.scope_path("one", Some(&n.to_string())).unwrap();
            state::write_json(&path, &"not even a cache map").unwrap();
        }
        cache.cooldown("one", Some("123"), 120).unwrap();
        cache
            .put("one", None, "latest", &Output::new("one", None))
            .unwrap();
        assert_eq!(
            std::fs::read_dir(cache.root.join("content"))
                .unwrap()
                .count(),
            MAX_SCOPES
        );
        assert!(cache.get("one", None, "latest", 100).unwrap().is_some());
        assert_eq!(
            cache.check_cooldown("one", Some("123")).unwrap_err().kind,
            Kind::RateLimit
        );
    }
    #[test]
    fn freshness_boundary_age_future_and_zero_ttl() {
        let (_dir, cache) = fixture();
        let mut output = Output::new("one", None);
        output.provenance.retrieved_at = now() - 10;
        output.provenance.age_seconds = 999;
        cache.put("one", None, "k", &output).unwrap();
        let hit = cache.get("one", None, "k", 100).unwrap().unwrap();
        assert!(hit.provenance.age_seconds >= 10 && hit.provenance.age_seconds < 100);
        assert_eq!(hit.provenance.retrieved_at, output.provenance.retrieved_at);
        assert!(cache.get("one", None, "k", 10).unwrap().is_none());
        assert!(cache.get("one", None, "k", 0).unwrap().is_none());
        output.provenance.retrieved_at = now() + 100;
        assert!(cache.put("one", None, "k", &output).is_err());
        let entries = Entries::from([(digest("one", None, Some("k")).unwrap(), output)]);
        state::write_json(&cache.scope_path("one", None).unwrap(), &entries).unwrap();
        assert!(cache.get("one", None, "k", u64::MAX).unwrap().is_none());
    }
    #[test]
    fn validates_provenance_on_put_and_hit() {
        let (_dir, cache) = fixture();
        let output = Output::new("evil", Some("wrong".into()));
        assert!(cache.put("one", Some("123"), "k", &output).is_err());
        let entries = Entries::from([(digest("one", Some("123"), Some("k")).unwrap(), output)]);
        state::write_json(&cache.scope_path("one", Some("123")).unwrap(), &entries).unwrap();
        assert!(cache.get("one", Some("123"), "k", 100).is_err());
    }
    #[test]
    fn cooldown_persists_preserves_max_and_survives_purge() {
        let (_dir, cache) = fixture();
        cache.cooldown("one", Some("123"), 120).unwrap();
        let path = cache.root.join("cooldowns.json");
        let before: Cooldowns = state::read_json(&path).unwrap().unwrap();
        cache.cooldown("one", Some("123"), 1).unwrap();
        assert_eq!(
            state::read_json::<Cooldowns>(&path).unwrap().unwrap(),
            before
        );
        let other = Cache::new(cache.root.clone());
        let error = other.check_cooldown("one", Some("123")).unwrap_err();
        assert_eq!(error.kind, Kind::RateLimit);
        assert!(error.retry_after_seconds.is_some_and(|n| n > 0 && n <= 120));
        other.check_cooldown("two", Some("123")).unwrap();
        other.check_cooldown("one", Some("456")).unwrap();
        other.check_cooldown("one", None).unwrap();
        cache
            .put("one", None, "k", &Output::new("one", None))
            .unwrap();
        cache.purge().unwrap();
        assert!(cache.get("one", None, "k", 100).unwrap().is_none());
        assert!(cache.check_cooldown("one", Some("123")).is_err());
        let entries = Cooldowns::from([(digest("one", Some("123"), None).unwrap(), now() - 1)]);
        state::write_json(&path, &entries).unwrap();
        cache.check_cooldown("one", Some("123")).unwrap();
        cache.cooldown("one", None, 0).unwrap();
        cache.check_cooldown("one", None).unwrap();
    }
    #[test]
    fn cooldown_metadata_is_bounded_without_evicting_active_deadlines() {
        let (_dir, cache) = fixture();
        let path = cache.root.join("cooldowns.json");
        let mut entries = Cooldowns::new();
        for n in 0..MAX_COOLDOWNS {
            entries.insert(
                digest("one", Some(&n.to_string()), None).unwrap(),
                now() + 120,
            );
        }
        state::write_json(&path, &entries).unwrap();
        assert_eq!(
            cache.cooldown("one", None, 120).unwrap_err().kind,
            Kind::Storage
        );
        assert_eq!(
            state::read_json::<Cooldowns>(&path).unwrap().unwrap(),
            entries
        );
        cache.cooldown("one", Some("0"), 240).unwrap();
        assert!(
            cache
                .check_cooldown("one", Some("0"))
                .unwrap_err()
                .retry_after_seconds
                .unwrap()
                > 120
        );
        for deadline in entries.values_mut() {
            *deadline = now() - 1;
        }
        state::write_json(&path, &entries).unwrap();
        cache.cooldown("one", None, 120).unwrap();
        assert_eq!(
            state::read_json::<Cooldowns>(&path).unwrap().unwrap().len(),
            1
        );
    }
    #[test]
    fn concurrent_cooldowns_preserve_longest_and_puts_preserve_entries() {
        let (_dir, cache) = fixture();
        std::thread::scope(|scope| {
            for n in 1..=12 {
                let cache = &cache;
                scope.spawn(move || {
                    cache.cooldown("one", None, n * 100).unwrap();
                    cache
                        .put("one", None, &n.to_string(), &Output::new("one", None))
                        .unwrap();
                });
            }
        });
        assert!(
            cache
                .check_cooldown("one", None)
                .unwrap_err()
                .retry_after_seconds
                .unwrap()
                > 1100
        );
        for n in 1..=12 {
            assert!(
                cache
                    .get("one", None, &n.to_string(), 100)
                    .unwrap()
                    .is_some()
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn restrictive_permissions_and_symlink_rejection() {
        use std::{
            fs,
            os::unix::fs::{PermissionsExt, symlink},
        };
        let (_dir, cache) = fixture();
        cache
            .put("one", None, "k", &Output::new("one", None))
            .unwrap();
        cache.cooldown("one", None, 10).unwrap();
        assert_eq!(
            fs::metadata(&cache.root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for entry in fs::read_dir(&cache.root).unwrap() {
            let metadata = entry.unwrap().metadata().unwrap();
            assert_eq!(
                metadata.permissions().mode() & 0o777,
                if metadata.is_dir() { 0o700 } else { 0o600 }
            );
        }
        let path = cache.scope_path("one", None).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_file(&path).unwrap();
        symlink(cache.root.join("cooldowns.json"), &path).unwrap();
        assert!(cache.get("one", None, "k", 100).is_err());
        assert!(
            cache
                .put("one", None, "k", &Output::new("one", None))
                .is_err()
        );
        let cooldown_before = fs::read(cache.root.join("cooldowns.json")).unwrap();
        cache.purge().unwrap();
        assert!(path.symlink_metadata().is_err());
        assert_eq!(
            fs::read(cache.root.join("cooldowns.json")).unwrap(),
            cooldown_before
        );
        assert_eq!(
            cache.check_cooldown("one", None).unwrap_err().kind,
            Kind::RateLimit
        );
        fs::remove_file(cache.root.join(".cache.lock")).unwrap();
        symlink(
            cache.root.join("cooldowns.json"),
            cache.root.join(".cache.lock"),
        )
        .unwrap();
        assert!(cache.cooldown("one", None, 200).is_err());
    }
}
