use super::generation::{PublicFetch, WARNING};
use super::*;

fn corrupt_generation(h: &Harness) -> PathBuf {
    h.cache.purge().unwrap();
    let path = h.root.join("cache/generation.json");
    std::fs::write(&path, b"invalid synthetic generation").unwrap();
    path
}

#[test]
fn no_cache_bypasses_generation_capture_and_put_for_public_and_private() {
    for corrupt in [false, true] {
        let h = Harness::new();
        let path = if corrupt {
            corrupt_generation(&h)
        } else {
            h.root.join("cache/generation.json")
        };
        let public = ["read", "20", "--no-cache"];
        let transport = PublicFetch::new();
        let out = h
            .run_with(&public, &NoAccess, &transport, &NoAccess)
            .unwrap();
        assert_eq!(out.posts.len(), 1);
        assert!(!out.warnings.iter().any(|w| w == WARNING));
        assert!(h.stored(&public, "fxtwitter", None).is_none());
        let args = ["following", "list", "--account", "work", "--no-cache"];
        let graph = FakeFactory::new(h.events.clone());
        graph.page(None);
        let out = h.run(&args, &graph).unwrap();
        assert_users(&out, "miss");
        assert!(h.stored(&args, "graphql", Some(ACTOR)).is_none());
        if corrupt {
            assert_eq!(
                std::fs::read(path).unwrap(),
                b"invalid synthetic generation"
            );
        } else {
            assert!(!path.exists());
            assert!(!h.root.join("cache/.cache.lock").exists());
        }
    }
}

#[test]
fn refresh_and_ttl_zero_skip_hits_but_write_only_with_matching_generation() {
    for flags in [vec!["--refresh"], vec!["--cache-ttl", "0"]] {
        for invalidate in [false, true] {
            let h = Harness::new();
            let mut args = vec!["following", "list", "--account", "work"];
            args.extend(&flags);
            let mut old = Output::new("graphql", Some(ACTOR.into()));
            old.users = Some(vec![identity("999", "old_fixture")]);
            h.seed(&args, &old);
            let graph = FakeFactory::new(h.events.clone());
            graph.page(None);
            if invalidate {
                let cache = h.cache.clone();
                *graph.content_hook.borrow_mut() = Some(Box::new(move || {
                    cache.invalidate_scope("graphql", Some(ACTOR)).unwrap()
                }));
            }
            let out = h.run(&args, &graph).unwrap();
            assert_users(&out, "miss");
            assert_eq!(out.warnings.iter().any(|w| w == WARNING), invalidate);
            assert_eq!(
                h.stored(&args, "graphql", Some(ACTOR)).is_some(),
                !invalidate
            );

            let mut public = vec!["read", "20"];
            public.extend(&flags);
            h.seed(&public, &Output::new("fxtwitter", None));
            let transport = PublicFetch::new();
            if invalidate {
                let cache = h.cache.clone();
                *transport.hook.borrow_mut() = Some(Box::new(move || cache.purge().unwrap()));
            }
            let out = h
                .run_with(&public, &NoAccess, &transport, &NoAccess)
                .unwrap();
            assert_eq!(out.posts.len(), 1);
            assert_eq!(*transport.calls.borrow(), 1);
            assert_eq!(out.warnings.iter().any(|w| w == WARNING), invalidate);
            assert_eq!(h.stored(&public, "fxtwitter", None).is_some(), !invalidate);
        }
    }
}

#[test]
fn corrupt_generation_blocks_misses_not_hits_and_follows_identity_and_handle_checks() {
    let h = Harness::new();
    corrupt_generation(&h);
    let public = ["read", "20"];
    assert_eq!(
        h.run_with(&public, &NoAccess, &NoAccess, &NoAccess)
            .unwrap_err()
            .kind,
        Kind::Storage
    );
    h.seed(&public, &Output::new("fxtwitter", None));
    assert_eq!(
        h.run_with(&public, &NoAccess, &NoAccess, &NoAccess)
            .unwrap()
            .provenance
            .cache,
        "hit"
    );
    let args = ["following", "list", "--account", "work"];
    let mut graph = FakeFactory::new(h.events.clone());
    for (viewer, selector, expected) in [
        (
            identity(OTHER, "other_fixture"),
            "work",
            Kind::IdentityMismatch,
        ),
        (
            identity(ACTOR, "renamed_fixture"),
            "@fixture",
            Kind::InvalidInput,
        ),
        (identity(ACTOR, "fixture"), "work", Kind::Storage),
    ] {
        graph.viewer = Ok(viewer);
        assert_eq!(
            h.run(&["following", "list", "--account", selector], &graph)
                .unwrap_err()
                .kind,
            expected
        );
        h.expect_events(verified());
    }
    let mut out = Output::new("graphql", Some(ACTOR.into()));
    out.users = Some(vec![]);
    h.seed(&args, &out);
    assert_eq!(h.run(&args, &graph).unwrap().provenance.cache, "hit");
    h.expect_events(verified());
}

#[test]
fn first_and_partial_request_failures_are_not_cached_even_when_epoch_changes() {
    struct FailedPublic;
    impl Transport for FailedPublic {
        fn get(&self, _: Request) -> Result<Response> {
            Err(Error::new(Kind::Network, "Synthetic failure"))
        }
    }
    let h = Harness::new();
    let public = ["read", "20"];
    assert_eq!(
        h.run_with(&public, &NoAccess, &FailedPublic, &NoAccess)
            .unwrap_err()
            .kind,
        Kind::Network
    );
    assert!(h.stored(&public, "fxtwitter", None).is_none());
    for partial in [false, true] {
        let args = ["following", "list", "--account", "work", "--max-pages", "2"];
        let graph = FakeFactory::new(h.events.clone());
        if partial {
            graph.page(Some("next"));
        }
        graph
            .pages
            .borrow_mut()
            .push_back(Err(Error::new(Kind::Network, "Synthetic failure")));
        let cache = h.cache.clone();
        *graph.content_hook.borrow_mut() = Some(Box::new(move || cache.purge().unwrap()));
        let result = h.run(&args, &graph);
        if partial {
            let out = result.unwrap();
            assert_users(&out, "miss");
            assert!(out.request_failed);
            assert!(!out.warnings.iter().any(|w| w == WARNING));
        } else {
            assert_eq!(result.unwrap_err().kind, Kind::Network);
        }
        assert!(h.stored(&args, "graphql", Some(ACTOR)).is_none());
    }
}
