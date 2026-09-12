use super::*;
use std::{sync::mpsc, time::Duration};

pub(super) const WARNING: &str = "Cache generation changed during retrieval; result not stored";

// The fake content request cannot finish until a separate local purge has
// completed. The timeout is only a deadlock guard, not timing-based ordering.
fn purge_during_fetch(cache: Cache, global: bool) {
    let (done, completed) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = if global {
            cache.purge()
        } else {
            cache.invalidate_scope("graphql", Some(ACTOR))
        };
        done.send(result).unwrap();
    });
    completed
        .recv_timeout(Duration::from_secs(30))
        .expect("purge must not wait for content fetch to return")
        .unwrap();
    worker.join().unwrap();
}

pub(super) struct PublicFetch {
    pub calls: RefCell<u32>,
    pub hook: RefCell<Option<Box<dyn FnOnce()>>>,
    pub parents: bool,
}
impl PublicFetch {
    pub fn new() -> Self {
        Self {
            calls: RefCell::new(0),
            hook: RefCell::new(None),
            parents: false,
        }
    }
}
impl Transport for PublicFetch {
    fn get(&self, request: Request) -> Result<Response> {
        assert!(request.headers.is_empty());
        let id = request
            .url
            .strip_prefix("https://api.fxtwitter.com/2/status/")
            .unwrap();
        assert!(id == "20" || (self.parents && id == "21"));
        *self.calls.borrow_mut() += 1;
        if let Some(hook) = self.hook.take() {
            hook();
        }
        Ok(Response {
            status: 200,
            body: serde_json::to_vec(&serde_json::json!({"code":200,"status":{
                "type":"status","id":id,"text":"Synthetic public post",
                "author":{"id":"456","screen_name":"public_fixture"},
                "replying_to": if self.parents && id == "20" { Some(serde_json::json!({"status":"21"})) } else { None }
            }})).unwrap(),
            retry_after: None,
        })
    }
}

#[test]
fn public_purge_during_first_content_request_retains_post_and_parent_results() {
    for global in [false, true] {
        for parents in [false, true] {
            let h = Harness::new();
            let args = [if parents { "thread" } else { "read" }, "20"];
            let mut transport = PublicFetch::new();
            transport.parents = parents;
            let cache = h.cache.clone();
            *transport.hook.borrow_mut() =
                Some(Box::new(move || purge_during_fetch(cache, global)));
            let out = h.run_with(&args, &NoAccess, &transport, &NoAccess).unwrap();
            assert_eq!(out.posts.len(), if parents { 2 } else { 1 });
            assert!(out.complete && !out.request_failed);
            assert!(out.warnings.iter().any(|w| w == WARNING));
            assert_eq!(*transport.calls.borrow(), if parents { 2 } else { 1 }); // no retry
            assert!(h.stored(&args, "fxtwitter", None).is_none());
            let fresh = h.run_with(&args, &NoAccess, &transport, &NoAccess).unwrap();
            assert!(!fresh.warnings.iter().any(|w| w == WARNING));
            assert!(h.stored(&args, "fxtwitter", None).is_some());
            assert_eq!(
                h.run_with(&args, &NoAccess, &NoAccess, &NoAccess)
                    .unwrap()
                    .provenance
                    .cache,
                "hit"
            );
            h.expect_events(vec![]);
        }
    }
}

#[test]
fn authenticated_purge_during_content_retains_typed_data_and_new_invocation_can_cache() {
    for global in [false, true] {
        for args in [
            vec!["read", "20", "--account", "work"],
            vec!["following", "list", "--account", "work"],
            vec!["lists", "members", "list", "456", "--account", "work"],
            vec!["lists", "show", "456", "--account", "work"],
        ] {
            let h = Harness::new();
            let graph = FakeFactory::new(h.events.clone());
            let users = args[0] == "following" || args[1] == "members";
            if users {
                graph.page(None);
            }
            let cache = h.cache.clone();
            let events = h.events.clone();
            *graph.content_hook.borrow_mut() = Some(Box::new(move || {
                assert_eq!(&events.borrow()[..3], &verified());
                purge_during_fetch(cache, global);
            }));
            let out = h.run(&args, &graph).unwrap();
            assert!(!out.request_failed);
            assert!(out.warnings.iter().any(|w| w == WARNING));
            assert_eq!(out.provenance.account_id.as_deref(), Some(ACTOR));
            assert_eq!(out.pages, 1);
            if users {
                assert_users(&out, "miss");
            } else if args[0] == "lists" {
                assert_eq!(out.lists.as_ref().unwrap()[0].id, "456");
            } else {
                assert_eq!(out.posts[0].id, "20");
            }
            assert!(h.stored(&args, "graphql", Some(ACTOR)).is_none());
            assert_eq!(h.events.borrow().len(), 4); // exactly one content request
            if users {
                graph.page(None);
            }
            let fresh = h.run(&args, &graph).unwrap();
            assert!(!fresh.warnings.iter().any(|w| w == WARNING));
            assert!(h.stored(&args, "graphql", Some(ACTOR)).is_some());
            assert!(h.stored(&args, "graphql", Some(OTHER)).is_none());
            assert!(h.stored(&args, "fxtwitter", None).is_none());
            assert_eq!(h.run(&args, &graph).unwrap().provenance.cache, "hit");
        }
    }
}

#[test]
fn generation_is_captured_after_viewer_not_before_authentication() {
    let h = Harness::new();
    let graph = FakeFactory::new(h.events.clone());
    let cache = h.cache.clone();
    *graph.viewer_hook.borrow_mut() = Some(Box::new(move || purge_during_fetch(cache, true)));
    let args = ["following", "list", "--account", "work"];
    graph.page(None);
    let out = h.run(&args, &graph).unwrap();
    assert_users(&out, "miss");
    assert!(!out.warnings.iter().any(|w| w == WARNING));
    assert!(h.stored(&args, "graphql", Some(ACTOR)).is_some());
}
