use super::*;
use crate::{pagination, transport::Response};
use std::cell::Cell;

fn response() -> Value {
    json!({"data":{"list":{"members_timeline":{"timeline":{"instructions":[
        {"type":"TimelineAddEntries","entries":[{"content":{"itemContent":{"itemType":"TimelineUser","user_results":{"result":{"__typename":"User","rest_id":"789","core":{"screen_name":"fixture"}}}}}}]},
        {"type":"TimelineReplaceEntry","entry":{"content":{"cursorType":"Bottom","value":"opaque + /= next"}}}
    ]}}}}})
}
struct Members {
    calls: Cell<u32>,
    fail_second: bool,
}
impl Transport for Members {
    fn get(&self, request: Request) -> Result<Response> {
        let n = self.calls.get();
        self.calls.set(n + 1);
        assert!(n < 2, "no extra retry");
        let url = url::Url::parse(&request.url).unwrap();
        assert_eq!(url.host_str(), Some("x.com"));
        assert_eq!(url.scheme(), "https");
        assert_eq!(
            url.path(),
            "/i/api/graphql/ljlktihgwXeYTfHwwiPj5A/ListMembers"
        );
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert!(!pairs.contains_key("fieldToggles"));
        let mut expected = json!({"listId":"456","count":5});
        if n > 0 {
            expected["cursor"] = json!("opaque + /= next");
        }
        assert_eq!(
            serde_json::from_str::<Value>(&pairs["variables"]).unwrap(),
            expected
        );
        let features: Value = serde_json::from_str(&pairs["features"]).unwrap();
        let names: std::collections::BTreeSet<_> = features
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let expected: std::collections::BTreeSet<_> =
            include_str!("../../operations/bookmark-features.txt")
                .lines()
                .collect();
        assert_eq!(names, expected);
        let failed = n > 0 && self.fail_second;
        Ok(Response {
            status: if failed { 429 } else { 200 },
            retry_after: Some(42),
            body: serde_json::to_vec(&if failed {
                json!({"message":"SYNTHETIC_SECRET"})
            } else {
                response()
            })
            .unwrap(),
        })
    }
}
#[test]
fn list_members_request_root_dedup_and_partial_failure() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for failed in [false, true] {
        let mock = Members {
            calls: Cell::new(0),
            fail_second: failed,
        };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let out = pagination::collect_users(
            Output::new("graphql", Some("123".into())),
            5,
            None,
            |cursor| graph.users_page(Operation::ListMembers, "456", 5, cursor),
        )
        .unwrap();
        assert_eq!(out.users.unwrap()[0].id, "789");
        assert!(out.posts.is_empty() && !out.complete);
        assert_eq!(out.request_failed, failed);
        assert_eq!(mock.calls.get(), 2);
        assert_eq!(out.provenance.account_id.as_deref(), Some("123"));
    }
}
#[test]
fn list_members_empty_and_missing_roots_are_distinct() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let mock = Mock {
        status: 200,
        body: json!({"data":{"list":{"members_timeline":{"timeline":{"instructions":[{"type":"TimelineAddEntries","entries":[]}]}}}}}),
    };
    let graph = Graphql {
        transport: &mock,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    assert!(
        graph
            .users_page(Operation::ListMembers, "456", 5, None)
            .unwrap()
            .users
            .is_empty()
    );
    for (status, body, kind) in [
        (200, json!({"data":{"list":null}}), Kind::ProtocolChanged),
        (401, json!({}), Kind::Authentication),
        (403, json!({}), Kind::Permission),
        (404, json!({}), Kind::ProtocolChanged),
        (429, json!({}), Kind::RateLimit),
        (503, json!({}), Kind::Network),
        (
            200,
            json!({"errors":[{"code":179,"message":"SYNTHETIC_SECRET"}]}),
            Kind::Permission,
        ),
    ] {
        let mock = Mock { status, body };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let e = graph
            .users_page(Operation::ListMembers, "456", 5, None)
            .err()
            .unwrap();
        assert_eq!(e.kind, kind);
        assert!(
            !serde_json::to_string(&e)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
}
