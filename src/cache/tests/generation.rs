use super::*;
use crate::cache::ConditionalPut::{Applied, GenerationChanged};
use serde_json::json;
use std::fs;

fn metadata(cache: &Cache) -> PathBuf {
    cache.root.join("generation.json")
}
fn value(cache: &Cache) -> u64 {
    state::read_json::<serde_json::Value>(&metadata(cache))
        .unwrap()
        .unwrap()["generation"]
        .as_u64()
        .unwrap()
}
fn put(cache: &Cache, token: &CacheGeneration) -> Result<ConditionalPut> {
    cache.put_if_generation(
        token,
        "graphql",
        Some("123"),
        "key",
        &Output::new("graphql", Some("123".into())),
    )
}

#[test]
fn initial_zero_matching_put_and_cross_instance_token_release_the_lock() {
    let (_dir, cache) = fixture();
    let token = cache.capture_generation().unwrap();
    assert!(!metadata(&cache).exists());
    // A persisted zero matches a missing-file capture. No per-account metadata.
    state::write_json(&metadata(&cache), &json!({"version":1,"generation":0})).unwrap();
    let same_root = Cache::new(cache.root.clone());
    assert_eq!(put(&same_root, &token).unwrap(), Applied);
    assert!(
        same_root
            .get("graphql", Some("123"), "key", 100)
            .unwrap()
            .is_some()
    );
    assert_eq!(value(&cache), 0);
    assert_eq!(fs::read_dir(&cache.root).unwrap().count(), 3); // lock, generation, content
}

#[test]
fn scope_and_global_purge_fence_old_tokens_and_allow_new_invocations() {
    for global in [false, true] {
        let (_dir, cache) = fixture();
        let before = cache.capture_generation().unwrap();
        assert_eq!(put(&cache, &before).unwrap(), Applied);
        for name in ["config.json", "journal.json"] {
            state::write_json(&cache.root.join(name), &"synthetic marker").unwrap();
        }
        cache.cooldown("graphql", Some("123"), 3600).unwrap();
        let preserved: Vec<_> = ["config.json", "journal.json", "cooldowns.json"]
            .map(|name| {
                (
                    cache.root.join(name),
                    fs::read(cache.root.join(name)).unwrap(),
                )
            })
            .into();
        state::write_json(&cache.root.join("cache.json"), &"legacy content").unwrap();
        if global {
            cache.purge().unwrap();
        } else {
            cache.invalidate_scope("graphql", Some("123")).unwrap();
        }
        assert_eq!(value(&cache), 1);
        assert_eq!(put(&cache, &before).unwrap(), GenerationChanged);
        assert!(
            cache
                .get("graphql", Some("123"), "key", 100)
                .unwrap()
                .is_none()
        );
        assert_eq!(cache.root.join("cache.json").exists(), !global);
        for (path, bytes) in &preserved {
            assert_eq!(&fs::read(path).unwrap(), bytes);
        }
        let after = cache.capture_generation().unwrap();
        assert_eq!(put(&cache, &after).unwrap(), Applied);
        // Further invalidation cannot reset/reuse a generation (including empty scope).
        cache.invalidate_scope("graphql", Some("missing")).unwrap();
        assert_eq!(value(&cache), 2);
        assert_eq!(put(&cache, &after).unwrap(), GenerationChanged);
    }
}

#[test]
fn scoped_epoch_suppresses_unrelated_writes_but_preserves_existing_hits_and_bytes() {
    let (_dir, cache) = fixture();
    let token = cache.capture_generation().unwrap();
    for (backend, account) in [("fxtwitter", None), ("graphql", Some("456"))] {
        cache
            .put(
                backend,
                account,
                "key",
                &Output::new(backend, account.map(str::to_owned)),
            )
            .unwrap();
    }
    let paths = [
        cache.scope_path("fxtwitter", None).unwrap(),
        cache.scope_path("graphql", Some("456")).unwrap(),
    ];
    let before = paths.each_ref().map(|path| fs::read(path).unwrap());
    cache.invalidate_scope("graphql", Some("123")).unwrap();
    for ((backend, account), (path, bytes)) in [("fxtwitter", None), ("graphql", Some("456"))]
        .into_iter()
        .zip(paths.iter().zip(before))
    {
        assert_eq!(
            cache
                .put_if_generation(
                    &token,
                    backend,
                    account,
                    "key",
                    &Output::new(backend, account.map(str::to_owned))
                )
                .unwrap(),
            GenerationChanged
        );
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert!(cache.get(backend, account, "key", 100).unwrap().is_some());
    }
}

#[test]
fn malformed_version_counter_and_overflow_block_before_deletion_without_reset() {
    for bytes in [
        "{bad",
        "null",
        "{}",
        "{\"version\":2,\"generation\":0}",
        "{\"version\":1,\"generation\":-1}",
        "{\"version\":1,\"generation\":1.0}",
        "{\"version\":1,\"generation\":0,\"extra\":1}",
        "{\"version\":1,\"generation\":18446744073709551616}",
        "{\"version\":1,\"generation\":18446744073709551615}",
    ] {
        let (_dir, cache) = fixture();
        let token = cache.capture_generation().unwrap();
        put(&cache, &token).unwrap();
        let content = cache.scope_path("graphql", Some("123")).unwrap();
        let before = fs::read(&content).unwrap();
        state::write_json(&cache.root.join("cache.json"), &"legacy marker").unwrap();
        fs::write(metadata(&cache), bytes).unwrap();
        for global in [false, true] {
            let error = if global {
                cache.purge()
            } else {
                cache.invalidate_scope("graphql", Some("123"))
            }
            .unwrap_err();
            assert_eq!(error.kind, Kind::Storage);
            assert_eq!(error.message, storage().message);
            assert_eq!(fs::read(&content).unwrap(), before);
            assert!(cache.root.join("cache.json").exists());
            assert_eq!(fs::read(metadata(&cache)).unwrap(), bytes.as_bytes());
        }
        if bytes.contains("18446744073709551615") {
            // Maximum is a valid readable generation, but cannot be advanced.
            assert!(cache.capture_generation().is_ok());
        } else {
            assert_eq!(cache.capture_generation().unwrap_err().kind, Kind::Storage);
            assert_eq!(put(&cache, &token).unwrap_err().kind, Kind::Storage);
        }
    }
}

#[test]
fn foreign_root_rejected_and_matching_put_retains_validation_and_size_policy() {
    let (_one, cache) = fixture();
    let (_two, other) = fixture();
    let token = cache.capture_generation().unwrap();
    assert_eq!(put(&other, &token).unwrap_err().kind, Kind::Storage);
    assert!(!other.root.join("content").exists());
    for mut out in [
        Output::new("wrong", None),
        Output::new("graphql", Some("456".into())),
        Output::new("graphql", Some("123".into())),
    ] {
        if out.provenance.account_id.as_deref() == Some("123") {
            out.provenance.retrieved_at = now() + 1000;
        }
        assert_eq!(
            cache
                .put_if_generation(&token, "graphql", Some("123"), "key", &out)
                .unwrap_err()
                .kind,
            Kind::Storage
        );
    }
    put(&cache, &token).unwrap();
    let mut large = Output::new("graphql", Some("123".into()));
    large.warnings.push("x".repeat(MAX_SCOPE_BYTES));
    assert_eq!(
        cache
            .put_if_generation(&token, "graphql", Some("123"), "key", &large)
            .unwrap(),
        Applied
    );
    assert!(
        cache
            .get("graphql", Some("123"), "key", 100)
            .unwrap()
            .is_none()
    );
    assert_eq!(put(&cache, &token).unwrap(), Applied);
}

#[test]
fn concurrent_invalidations_do_not_lose_generation_increments() {
    let (_dir, cache) = fixture();
    let token = cache.capture_generation().unwrap();
    std::thread::scope(|threads| {
        for n in 0..8 {
            let cache = &cache;
            threads.spawn(move || {
                if n % 2 == 0 {
                    cache.purge().unwrap();
                } else {
                    cache
                        .invalidate_scope("graphql", Some(&n.to_string()))
                        .unwrap();
                }
            });
        }
    });
    assert_eq!(value(&cache), 8);
    assert_eq!(put(&cache, &token).unwrap(), GenerationChanged);
    assert_eq!(
        put(&cache, &cache.capture_generation().unwrap()).unwrap(),
        Applied
    );
}

#[test]
fn matching_put_storage_failure_is_not_a_generation_change() {
    let (_dir, cache) = fixture();
    let token = cache.capture_generation().unwrap();
    let path = cache.scope_path("graphql", Some("123")).unwrap();
    state::write_json(&path, &"malformed synthetic scope").unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(put(&cache, &token).unwrap_err().kind, Kind::Storage);
    assert_eq!(fs::read(&path).unwrap(), before);
    cache.invalidate_scope("graphql", Some("456")).unwrap();
    assert_eq!(put(&cache, &token).unwrap(), GenerationChanged);
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn deletion_failure_conservatively_advances_before_unlink() {
    for global in [false, true] {
        let (_dir, cache) = fixture();
        let token = cache.capture_generation().unwrap();
        put(&cache, &token).unwrap();
        let path = cache.scope_path("graphql", Some("123")).unwrap();
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        let error = if global {
            cache.purge()
        } else {
            cache.invalidate_scope("graphql", Some("123"))
        }
        .unwrap_err();
        assert_eq!(error.kind, Kind::Storage);
        assert_eq!(value(&cache), 1);
        // A mismatching put never even opens corrupt/unsupported scope content.
        assert_eq!(put(&cache, &token).unwrap(), GenerationChanged);
        assert!(path.is_dir());
    }
}
