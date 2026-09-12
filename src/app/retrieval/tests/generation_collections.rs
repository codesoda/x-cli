use super::generation::WARNING;
use super::*;

fn queue_page(graph: &FakeFactory, inventory: bool) {
    // Normalized synthetic data from the same fake, prepared before retrieval.
    let fixture = FakeReadGraph(graph);
    if inventory {
        graph.list_pages.borrow_mut().push_back(Ok(ListsPage {
            lists: fixture.list_metadata("456", ACTOR).unwrap().lists.unwrap(),
            next_cursor: None,
            warnings: vec![],
        }));
    } else {
        graph.post_pages.borrow_mut().push_back(Ok(Page {
            posts: fixture.read("22", ACTOR).unwrap().posts,
            next_cursor: None,
            warnings: vec![],
        }));
    }
    graph.events.take();
}

#[test]
fn inventory_and_post_collections_capture_before_first_content_including_user_lookup() {
    for (mut args, expected) in [
        (vec!["lists", "list"], vec![Event::Lists(ACTOR.into())]),
        (
            vec!["lists", "posts", "456"],
            vec![Event::Posts(Operation::ListPosts, "456".into())],
        ),
        (
            vec!["search", "fixture"],
            vec![Event::Posts(Operation::Search, "fixture".into())],
        ),
        (
            vec!["user", "posts", "other_fixture"],
            vec![
                Event::Lookup("other_fixture".into()),
                Event::Posts(Operation::Timeline, OTHER.into()),
            ],
        ),
        (
            vec!["bookmarks", "list"],
            vec![Event::Posts(Operation::Bookmarks, "".into())],
        ),
        (
            vec!["likes", "list"],
            vec![Event::Posts(Operation::Likes, ACTOR.into())],
        ),
        (
            vec!["thread", "20", "--replies"],
            vec![
                Event::Read("20".into(), ACTOR.into()),
                Event::Posts(Operation::Detail, "20".into()),
            ],
        ),
    ] {
        args.extend(["--account", "work"]);
        let h = Harness::new();
        let graph = FakeFactory::new(h.events.clone());
        let inventory = args[..2] == ["lists", "list"];
        queue_page(&graph, inventory);
        let cache = h.cache.clone();
        let events = h.events.clone();
        let first = expected[0].clone();
        *graph.content_hook.borrow_mut() = Some(Box::new(move || {
            assert_eq!(events.borrow().last(), Some(&first));
            // Invoked from within the FIRST content/lookup call, not after a page.
            cache.invalidate_scope("graphql", Some(ACTOR)).unwrap();
        }));
        let out = h.run(&args, &graph).unwrap();
        assert!(!out.complete && !out.request_failed);
        assert_eq!(out.provenance.account_id.as_deref(), Some(ACTOR));
        assert!(out.warnings.iter().any(|w| w == WARNING));
        assert!(h.stored(&args, "graphql", Some(ACTOR)).is_none());
        if inventory {
            assert_eq!(out.lists.as_ref().unwrap()[0].id, "456");
        } else {
            assert!(out.posts.iter().any(|post| post.id == "22"));
        }
        let mut full = verified();
        full.extend(expected);
        h.expect_events(full);
        queue_page(&graph, inventory);
        let fresh = h.run(&args, &graph).unwrap();
        assert!(!fresh.request_failed);
        assert!(!fresh.warnings.iter().any(|w| w == WARNING));
        assert!(h.stored(&args, "graphql", Some(ACTOR)).is_some());
        assert!(h.stored(&args, "graphql", Some(OTHER)).is_none());
        assert!(h.stored(&args, "fxtwitter", None).is_none());
    }
}
