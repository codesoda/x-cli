use super::*;

const ARGS: &[&str] = &["following", "list", "--account", "work"];

fn populated() -> Output {
    let mut out = Output::new("graphql", Some(ACTOR.into()));
    out.users = Some(vec![identity(OTHER, "other_fixture")]);
    out.pages = 1;
    out.stop_reason = "cursor_exhausted".into();
    out
}

#[test]
fn mismatched_viewer_precedes_populated_and_corrupt_authenticated_content() {
    for corrupt in [false, true] {
        let h = Harness::new();
        h.seed(ARGS, &populated());
        assert_users(&h.stored(ARGS, "graphql", Some(ACTOR)).unwrap(), "hit");
        if corrupt {
            h.corrupt_only_content();
        }
        let mut graph = FakeFactory::new(h.events.clone());
        graph.viewer = Ok(identity(OTHER, "other_fixture"));
        let error = h.run(ARGS, &graph).unwrap_err();
        assert_eq!(error.kind, Kind::IdentityMismatch);
        assert_eq!(error.exit_code(), 10);
        // Exact static error contract: no Viewer/registered IDs or cached data.
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({
                "kind": "identity_mismatch",
                "message": "Browser account does not match the connected stable identity",
                "retry_after_seconds": null
            })
        );
        h.expect_events(verified());
        if corrupt {
            // Prove the file really would fail if the identity gate were reordered.
            assert_eq!(
                h.cache
                    .get("graphql", Some(ACTOR), &key(ARGS), 86400)
                    .unwrap_err()
                    .kind,
                Kind::Storage
            );
        } else {
            assert_users(&h.stored(ARGS, "graphql", Some(ACTOR)).unwrap(), "hit");
        }
        assert_eq!(
            Config::load(&h.root)
                .unwrap()
                .resolve(Some("work"), None)
                .unwrap()
                .identity
                .id,
            ACTOR
        );
    }
}

#[test]
fn expired_viewer_and_bootstrap_failure_never_read_cached_or_upstream_content() {
    for stage in ["viewer", "bootstrap"] {
        let h = Harness::new();
        h.seed(ARGS, &populated());
        h.corrupt_only_content();
        let mut graph = FakeFactory::new(h.events.clone());
        let (kind, expected) = if stage == "viewer" {
            graph.viewer = Err(Error::new(Kind::Authentication, "Synthetic expired Viewer"));
            (Kind::Authentication, verified())
        } else {
            graph.bootstrap_error = Some(Kind::ProtocolChanged);
            (Kind::ProtocolChanged, vec![Event::Load, Event::Connect])
        };
        assert_eq!(h.run(ARGS, &graph).unwrap_err().kind, kind);
        h.expect_events(expected);
        assert!(h.stored(ARGS, "graphql", Some(OTHER)).is_none());
        assert!(h.stored(ARGS, "fxtwitter", None).is_none());
    }
}

#[test]
fn changed_explicit_handle_is_rejected_even_when_stable_id_matches() {
    let h = Harness::new();
    h.seed(ARGS, &populated());
    let mut graph = FakeFactory::new(h.events.clone());
    graph.viewer = Ok(identity(ACTOR, "renamed_fixture"));
    let error = h
        .run(&["following", "list", "--account", "@fixture"], &graph)
        .unwrap_err();
    assert_eq!(error.kind, Kind::InvalidInput);
    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.message,
        "Account handle changed; use its alias/connection or explicitly reconnect"
    );
    h.expect_events(verified());
    // Alias remains a stable-ID selector; rejecting a stale @handle is not an ID mismatch.
    assert_users(&h.run(ARGS, &graph).unwrap(), "hit");
    h.expect_events(verified());
    assert_eq!(
        Config::load(&h.root)
            .unwrap()
            .resolve(Some("work"), None)
            .unwrap()
            .identity
            .handle,
        "fixture"
    );
}

#[test]
fn later_page_rate_limit_retains_users_persists_cooldown_and_never_saves_failure() {
    let h = Harness::new();
    let args = ["following", "list", "--account", "work", "--max-pages", "3"];
    let graph = FakeFactory::new(h.events.clone());
    graph.page(Some("synthetic-next"));
    let mut rate = Error::new(Kind::RateLimit, "Synthetic rate limit");
    rate.retry_after_seconds = Some(3600);
    graph.pages.borrow_mut().push_back(Err(rate));
    let out = h.run(&args, &graph).unwrap();
    assert_users(&out, "miss");
    assert!(out.request_failed);
    assert_eq!(out.stop_reason, "request_failed");
    assert_eq!(out.next_cursor.as_deref(), Some("synthetic-next"));
    assert_eq!(
        out.warnings,
        [
            "Synthetic rate limit (exit code 6)",
            "Retry after 3600 seconds"
        ]
    );
    let mut events = verified();
    events.extend([
        users_event(Operation::Following, 20, None),
        users_event(Operation::Following, 20, Some("synthetic-next")),
    ]);
    h.expect_events(events);
    assert!(graph.pages.borrow().is_empty());
    assert!(h.stored(&args, "graphql", Some(ACTOR)).is_none());
    let persisted = Cache::new(h.root.join("cache"));
    let cooldown = persisted
        .check_cooldown("graphql", Some(ACTOR))
        .unwrap_err();
    assert_eq!(cooldown.kind, Kind::RateLimit);
    assert!(
        cooldown
            .retry_after_seconds
            .is_some_and(|seconds| seconds > 0 && seconds <= 3600)
    );
    assert!(persisted.check_cooldown("graphql", Some(OTHER)).is_ok());
    assert!(persisted.check_cooldown("fxtwitter", None).is_ok());
    // No retry, credential reload, connection, Viewer or fallback on the next invocation.
    let error = h
        .run_with(&args, &NoAccess, &NoAccess, &NoAccess)
        .unwrap_err();
    assert_eq!(error.kind, Kind::RateLimit);
    assert_eq!(error.exit_code(), 6);
    h.expect_events(vec![]);
}

#[test]
fn persisted_cooldown_precedes_credentials_even_with_populated_content() {
    let h = Harness::new();
    h.seed(ARGS, &populated());
    h.corrupt_only_content();
    h.cache.cooldown("graphql", Some(ACTOR), 3600).unwrap();
    assert_eq!(
        h.run_with(ARGS, &NoAccess, &NoAccess, &NoAccess)
            .unwrap_err()
            .kind,
        Kind::RateLimit
    );
    h.expect_events(vec![]);
}

#[test]
fn default_factory_still_hash_checks_public_bootstrap_before_viewer() {
    struct InvalidAsset(RefCell<u32>);
    impl Transport for InvalidAsset {
        fn get(&self, request: Request) -> Result<Response> {
            assert!(request.headers.is_empty(), "bootstrap must be anonymous");
            assert!(
                request.url == crate::providers::operations::BUNDLE_URL,
                "unexpected request"
            );
            *self.0.borrow_mut() += 1;
            Ok(Response {
                status: 200,
                body: b"synthetic non-matching asset".to_vec(),
                retry_after: None,
            })
        }
    }
    let h = Harness::new();
    h.seed(ARGS, &populated());
    h.corrupt_only_content();
    let cli = cli(ARGS);
    let (access, task) = Task::from_command(&cli.command).unwrap();
    let transport = InvalidAsset(RefCell::new(0));
    // Use the unchanged production entry, not the fake factory.
    let error = execute(
        &h.root,
        &h.cache,
        access,
        task,
        &transport,
        &SyntheticCredentials(h.events.clone()),
    )
    .unwrap_err();
    assert_eq!(error.kind, Kind::ProtocolChanged);
    assert_eq!(
        error.message,
        "Pinned X web-client asset changed; review operation manifest before continuing"
    );
    assert_eq!(*transport.0.borrow(), 1);
    h.expect_events(vec![Event::Load]);
}
