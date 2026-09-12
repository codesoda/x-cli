use super::*;

#[test]
fn relationships_miss_and_hit_verify_viewer_and_use_direction_and_actor() {
    let h = Harness::new();
    let graph = FakeFactory::new(h.events.clone());
    for (command, op) in [
        ("following", Operation::Following),
        ("followers", Operation::Followers),
    ] {
        let args = [command, "list", "--account", "work", "--page-size", "7"];
        assert!(h.stored(&args, "graphql", Some(ACTOR)).is_none());
        graph.page(None);
        let before = crate::model::now();
        let miss = h.run(&args, &graph).unwrap();
        assert_users(&miss, "miss");
        assert!(!miss.request_failed);
        assert_eq!(miss.stop_reason, "cursor_exhausted");
        assert!(miss.next_cursor.is_none());
        assert_eq!(miss.provenance.age_seconds, 0);
        assert!((before..=crate::model::now()).contains(&miss.provenance.retrieved_at));
        let mut events = verified();
        events.push(users_event(op, 7, None));
        h.expect_events(events);
        assert_users(&h.stored(&args, "graphql", Some(ACTOR)).unwrap(), "hit");
        assert!(h.stored(&args, "graphql", Some(OTHER)).is_none());
        assert!(h.stored(&args, "fxtwitter", None).is_none());

        // Reconstruct Cache in the shared runner via a second invocation; no
        // queued content response remains, so any request would also panic.
        let hit = h.run(&args, &graph).unwrap();
        assert_users(&hit, "hit");
        assert!(!hit.request_failed);
        assert_eq!(hit.stop_reason, miss.stop_reason);
        assert_eq!(hit.provenance.retrieved_at, miss.provenance.retrieved_at);
        assert!(hit.provenance.age_seconds <= crate::model::now() - before);
        h.expect_events(verified());
        assert!(graph.pages.borrow().is_empty());
    }
}

#[test]
fn legacy_wrong_shapes_refetch_after_viewer_instead_of_empty_success() {
    for (args, shape) in [
        (vec!["following", "list", "--account", "work"], "missing"),
        (vec!["followers", "list", "--account", "work"], "lists"),
        (vec!["lists", "show", "456", "--account", "work"], "users"),
        (vec!["read", "20", "--account", "work"], "lists"),
    ] {
        let h = Harness::new();
        let graph = FakeFactory::new(h.events.clone());
        let mut legacy = Output::new("graphql", Some(ACTOR.into()));
        match shape {
            "users" => legacy.users = Some(vec![]),
            "lists" => legacy.lists = Some(vec![]),
            _ => {}
        }
        h.seed(&args, &legacy);
        let task = Task::from_command(&cli(&args).command).unwrap().1;
        assert!(!task.accepts_cached(&h.stored(&args, "graphql", Some(ACTOR)).unwrap()));
        let mut events = verified();
        match args[0] {
            "following" | "followers" => {
                graph.page(None);
                events.push(users_event(
                    if args[0] == "following" {
                        Operation::Following
                    } else {
                        Operation::Followers
                    },
                    20,
                    None,
                ));
            }
            "lists" => events.push(Event::Metadata("456".into(), ACTOR.into())),
            _ => events.push(Event::Read("20".into(), ACTOR.into())),
        }
        let out = h.run(&args, &graph).unwrap();
        assert_eq!(out.provenance.cache, "miss");
        assert!(task.accepts_cached(&out));
        assert!(!out.request_failed);
        assert!(
            out.users.as_ref().is_some_and(|users| !users.is_empty())
                || out.lists.as_ref().is_some_and(|lists| !lists.is_empty())
                || !out.posts.is_empty()
        );
        h.expect_events(events);
        let stored = h.stored(&args, "graphql", Some(ACTOR)).unwrap();
        assert!(task.accepts_cached(&stored));
        assert_eq!(stored.users, out.users);
        assert_eq!(stored.lists, out.lists);
        assert_eq!(stored.posts.len(), out.posts.len());
    }
}

#[test]
fn list_owner_is_metadata_not_actor_for_next_read_or_cache_scope() {
    let h = Harness::new();
    let graph = FakeFactory::new(h.events.clone());
    let args = ["lists", "show", "456", "--account", "work"];
    let out = h.run(&args, &graph).unwrap();
    assert_eq!(
        out.lists.as_ref().unwrap()[0].owner.as_ref().unwrap().id,
        OTHER
    );
    assert_eq!(out.lists.as_ref().unwrap()[0].id, "456");
    assert!(out.posts.is_empty() && out.users.is_none());
    assert!(out.complete && !out.request_failed);
    assert_eq!(out.stop_reason, "single_list");
    assert_eq!(out.pages, 1);
    assert_eq!(out.provenance.cache, "miss");
    assert_eq!(out.provenance.account_id.as_deref(), Some(ACTOR));
    let mut events = verified();
    events.push(Event::Metadata("456".into(), ACTOR.into()));
    h.expect_events(events);
    assert_eq!(
        h.stored(&args, "graphql", Some(ACTOR)).unwrap().lists,
        out.lists
    );
    assert!(h.stored(&args, "graphql", Some(OTHER)).is_none());
    assert!(h.stored(&args, "fxtwitter", None).is_none());
    let hit = h.run(&args, &graph).unwrap();
    assert_eq!(hit.provenance.cache, "hit");
    assert_eq!(hit.lists, out.lists);
    h.expect_events(verified());

    let read_args = ["read", "20", "--account", "work"];
    let post = h.run(&read_args, &graph).unwrap();
    assert_eq!(post.posts[0].id, "20");
    assert_eq!(post.provenance.account_id.as_deref(), Some(ACTOR));
    let mut events = verified();
    events.push(Event::Read("20".into(), ACTOR.into()));
    h.expect_events(events);
    assert!(h.stored(&read_args, "graphql", Some(ACTOR)).is_some());
    assert!(h.stored(&read_args, "graphql", Some(OTHER)).is_none());
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
