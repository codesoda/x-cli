use super::*;
mod purge;
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
