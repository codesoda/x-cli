use super::*;
use crate::{pagination, transport::Response};
use std::cell::Cell;

fn user() -> Value {
    json!({"__typename":"User","rest_id":"456","core":{"screen_name":"fixture"}})
}
fn item() -> Value {
    json!({"itemType":"TimelineUser","user_results":{"result":user()}})
}
fn timeline() -> Value {
    json!({"data":{"user":{"result":{"__typename":"User","timeline":{"timeline":{"instructions":[
        {"type":"TimelineAddEntries","entries":[{"content":{"items":[{"item":{"itemContent":item()}}]}}]},
        {"type":"TimelineAddToModule","moduleItems":[{"item":{"itemContent":item()}}]},
        {"type":"TimelineReplaceEntry","entry":{"content":{"cursorType":"Bottom","value":"opaque + /= cursor"}}}
    ]}}}}}})
}
struct Relationships {
    op: Operation,
    calls: Cell<u32>,
    fail_second: bool,
}
impl Transport for Relationships {
    fn get(&self, request: Request) -> Result<Response> {
        let n = self.calls.get();
        self.calls.set(n + 1);
        assert!(n < 2, "unexpected retry");
        let url = url::Url::parse(&request.url).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("x.com"));
        let path = match self.op {
            Operation::Following => "/i/api/graphql/4EQGMEhtdVw8NeVBDQHESQ/Following",
            Operation::Followers => "/i/api/graphql/sF7aRC2fRq7OGOOp_qHntA/Followers",
            _ => unreachable!(),
        };
        assert_eq!(url.path(), path);
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert!(!params.contains_key("fieldToggles"));
        let mut expected = json!({"userId":"123","count":5,"includePromotedContent":false,"withGrokTranslatedBio":false});
        if n > 0 {
            expected["cursor"] = json!("opaque + /= cursor");
        }
        assert_eq!(
            serde_json::from_str::<Value>(&params["variables"]).unwrap(),
            expected
        );
        let features: Value = serde_json::from_str(&params["features"]).unwrap();
        let actual: std::collections::BTreeSet<_> = features
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let expected: std::collections::BTreeSet<_> =
            include_str!("../../operations/bookmark-features.txt")
                .lines()
                .collect();
        assert_eq!(actual, expected);
        assert!(request.headers.iter().any(|(name,value)|name=="content-type"&&value.as_str()=="application/json"));
        let failed = n > 0 && self.fail_second;
        Ok(Response {
            status: if failed { 429 } else { 200 },
            retry_after: Some(42),
            body: serde_json::to_vec(&if failed {
                json!({"message":"SYNTHETIC_SECRET"})
            } else {
                timeline()
            })
            .unwrap(),
        })
    }
}
#[test]
fn relationship_get_contract_dedup_and_partial_failures() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for op in [Operation::Following, Operation::Followers] {
        for failed in [false, true] {
            let mock = Relationships {
                op,
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
                |cursor| graph.users_page(op, "123", 5, cursor),
            )
            .unwrap();
            assert_eq!(out.users.as_ref().unwrap().len(), 1);
            assert_eq!(out.users.as_ref().unwrap()[0].id, "456");
            assert!(out.posts.is_empty() && !out.complete);
            assert_eq!(out.request_failed, failed);
            assert_eq!(mock.calls.get(), 2);
            assert!(
                !serde_json::to_string(&out)
                    .unwrap()
                    .contains("SYNTHETIC_SECRET")
            );
        }
    }
}
#[test]
fn relationship_parser_requires_current_core_and_user_item_shape() {
    let valid =
        json!([{"type":"TimelineAddEntries","entries":[{"content":{"itemContent":item()}}]}]);
    assert_eq!(
        users::parse_users_page(&valid).unwrap().users[0].handle,
        "fixture"
    );
    for result in [
        json!({"__typename":"UserUnavailable"}),
        json!({"__typename":"User","rest_id":"456","legacy":{"screen_name":"fixture"}}),
        json!({"__typename":"User","rest_id":456,"core":{"screen_name":"fixture"}}),
        json!({"__typename":"User","rest_id":"456","core":{"screen_name":"SYNTHETIC_SECRET/"}}),
    ] {
        let value = json!([{"type":"TimelineAddEntries","entries":[{"content":{"itemContent":{"itemType":"TimelineUser","user_results":{"result":result}}}}]}]);
        let error = users::parse_users_page(&value).err().unwrap();
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
    let empty =
        users::parse_users_page(&json!([{"type":"TimelineAddEntries","entries":[]}])).unwrap();
    assert!(empty.users.is_empty());
    assert!(users::parse_users_page(&json!([])).is_err());
    assert!(users::parse_users_page(&json!([{"type":"TimelineRemoveEntries"}])).is_err());
}
#[test]
fn relationship_promotion_metadata_never_bypasses_item_validation() {
    let wrap = |item: Value| json!([{"type":"TimelineAddEntries","entries":[{"content":{"itemContent":item}}]}]);
    let mut null_metadata = item();
    null_metadata["promotedMetadata"] = Value::Null;
    assert_eq!(
        users::parse_users_page(&wrap(null_metadata))
            .unwrap()
            .users
            .len(),
        1
    );
    for metadata in [json!({}), json!("SYNTHETIC_SECRET"), json!(false)] {
        let mut value = item();
        value["promotedMetadata"] = metadata;
        let error = users::parse_users_page(&wrap(value)).err().unwrap();
        assert_eq!(error.diagnostic, Some(Diagnostic::TimelineItem));
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
    assert!(
        users::parse_users_page(&wrap(json!({"itemType":"Unknown","promotedMetadata":null})))
            .is_err()
    );
}

#[test]
fn relationships_reject_bad_roots_and_http_errors() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let mut wrong = timeline();
    wrong["data"]["user"]["result"]["__typename"] = json!("UserUnavailable");
    for op in [Operation::Following, Operation::Followers] {
        for (status, body, kind) in [
            (200, wrong.clone(), Kind::ProtocolChanged),
            (200, json!({"data":{}}), Kind::ProtocolChanged),
            (
                401,
                json!({"message":"SYNTHETIC_SECRET"}),
                Kind::Authentication,
            ),
            (403, json!({}), Kind::Permission),
            (404, json!({}), Kind::ProtocolChanged),
            (429, json!({}), Kind::RateLimit),
            (503, json!({}), Kind::Network),
            (
                200,
                json!({"errors":[{"code":89,"message":"SYNTHETIC_SECRET"}]}),
                Kind::Authentication,
            ),
        ] {
            let mock = Mock { status, body };
            let graph = Graphql {
                transport: &mock,
                session: &session,
                bearer: Zeroizing::new("synthetic-public-token".into()),
            };
            let error = graph.users_page(op, "123", 5, None).err().unwrap();
            assert_eq!(error.kind, kind);
            assert!(
                !serde_json::to_string(&error)
                    .unwrap()
                    .contains("SYNTHETIC_SECRET")
            );
        }
    }
}
