use super::*;

struct PublicFixture(RefCell<u32>);
impl Transport for PublicFixture {
    fn get(&self, request: Request) -> Result<Response> {
        assert!(
            request.headers.is_empty(),
            "public requests must not carry credentials"
        );
        assert!(
            request.url == "https://api.fxtwitter.com/2/status/20",
            "unexpected public request"
        );
        *self.0.borrow_mut() += 1;
        Ok(Response {
            status: 200,
            body: br#"{"code":200,"status":{"type":"status","id":"20","text":"Synthetic public post","author":{"id":"456","screen_name":"public_fixture"},"replying_to":null}}"#.to_vec(),
            retry_after: None,
        })
    }
}

#[test]
fn public_miss_wrong_shape_and_hit_never_touch_authenticated_dependencies() {
    for shape in ["miss", "users", "lists"] {
        let h = Harness::new();
        let args = ["read", "20"];
        // A configured authenticated default, corrupt private content and an
        // active private cooldown must all remain irrelevant to public reads.
        assert_eq!(
            Config::load(&h.root)
                .unwrap()
                .resolve(None, None)
                .unwrap()
                .identity
                .id,
            ACTOR
        );
        h.seed(&args, &Output::new("graphql", Some(ACTOR.into())));
        h.corrupt_only_content();
        h.cache.cooldown("graphql", Some(ACTOR), 3600).unwrap();
        if shape != "miss" {
            let mut malformed = Output::new("fxtwitter", None);
            if shape == "users" {
                malformed.users = Some(vec![identity(OTHER, "other_fixture")]);
            } else {
                malformed.lists = Some(vec![]);
            }
            h.seed(&args, &malformed);
        }
        let transport = PublicFixture(RefCell::new(0));
        let out = h.run_with(&args, &NoAccess, &transport, &NoAccess).unwrap();
        assert_eq!(*transport.0.borrow(), 1);
        assert_eq!(out.provenance.backend, "fxtwitter");
        assert!(out.provenance.account_id.is_none());
        assert_eq!(out.provenance.cache, "miss");
        assert_eq!(out.provenance.age_seconds, 0);
        assert_eq!(out.posts.len(), 1);
        assert_eq!(out.posts[0].id, "20");
        assert_eq!(out.posts[0].text, "Synthetic public post");
        assert!(out.users.is_none() && out.lists.is_none());
        assert!(out.complete && !out.request_failed);
        assert_eq!(out.stop_reason, "single_post");
        assert_eq!(out.pages, 1);
        let stored = h.stored(&args, "fxtwitter", None).unwrap();
        assert_eq!(stored.provenance.cache, "hit");
        assert_eq!(stored.posts[0].text, out.posts[0].text);
        assert!(stored.users.is_none() && stored.lists.is_none());
        let hit = h.run_with(&args, &NoAccess, &NoAccess, &NoAccess).unwrap();
        assert_eq!(hit.provenance.cache, "hit");
        assert_eq!(hit.provenance.backend, "fxtwitter");
        assert!(hit.provenance.account_id.is_none());
        assert_eq!(hit.posts[0].text, out.posts[0].text);
        assert!(hit.complete && !hit.request_failed);
        h.expect_events(vec![]);
        assert_eq!(
            h.cache
                .get("graphql", Some(ACTOR), &key(&args), 86400)
                .unwrap_err()
                .kind,
            Kind::Storage
        );
        assert_eq!(
            h.cache
                .check_cooldown("graphql", Some(ACTOR))
                .unwrap_err()
                .kind,
            Kind::RateLimit
        );
    }
}
