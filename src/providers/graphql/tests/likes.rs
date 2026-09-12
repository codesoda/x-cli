use super::*;
use crate::{pagination, transport::Response};
use std::cell::Cell;

fn timeline() -> Value {
    json!({"data":{"user":{"result":{"__typename":"User","timeline":{"timeline":{"instructions":[
        {"type":"TimelineAddEntries","entries":[
            {"content":{"itemContent":{"tweet_results":{"result":tweet()}}}},
            {"content":{"cursorType":"Bottom","value":"opaque + / = next"}}
        ]}
    ]}}}}}})
}
struct Likes {
    calls: Cell<u32>,
}
impl Transport for Likes {
    fn get(&self, request: Request) -> Result<Response> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        assert!(call < 2, "must not retry a failed collection request");
        let url = url::Url::parse(&request.url).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("x.com"));
        assert_eq!(url.path(), "/i/api/graphql/o000A_Cp4JPOihhbeEgi0g/Likes");
        let parameters: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let variables: Value = serde_json::from_str(&parameters["variables"]).unwrap();
        let mut expected = json!({"userId":"123","count":5,"includePromotedContent":false,
            "withClientEventToken":false,"withBirdwatchNotes":false,"withVoice":false});
        if call > 0 {
            expected["cursor"] = json!("opaque + / = next");
        }
        assert_eq!(variables, expected);
        let toggles: Value = serde_json::from_str(&parameters["fieldToggles"]).unwrap();
        assert_eq!(toggles, json!({"withArticlePlainText":false}));
        let features: Value = serde_json::from_str(&parameters["features"]).unwrap();
        let expected_names: std::collections::BTreeSet<_> =
            include_str!("../../operations/bookmark-features.txt")
                .lines()
                .collect();
        let names: std::collections::BTreeSet<_> = features
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(names, expected_names);
        assert!(request.headers.iter().any(|(name, value)| name == "content-type" && value.as_str() == "application/json"));
        Ok(Response {
            status: if call == 0 { 200 } else { 429 },
            retry_after: Some(42),
            body: serde_json::to_vec(&if call == 0 {
                timeline()
            } else {
                json!({"message":"SYNTHETIC_SECRET"})
            })
            .unwrap(),
        })
    }
}

#[test]
fn own_likes_query_contract_and_partial_failure() {
    let mock = Likes {
        calls: Cell::new(0),
    };
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let graph = Graphql {
        transport: &mock,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    let output = pagination::collect(
        Output::new("graphql", Some("123".into())),
        3,
        None,
        |cursor| graph.page(Operation::Likes, "123", 5, cursor),
    )
    .unwrap();
    assert_eq!(output.posts.len(), 1);
    assert_eq!(output.posts[0].text, "long");
    assert_eq!(output.provenance.account_id.as_deref(), Some("123"));
    assert_eq!(output.pages, 1);
    assert!(output.request_failed && !output.complete);
    assert_eq!(mock.calls.get(), 2);
    assert!(
        !serde_json::to_string(&output)
            .unwrap()
            .contains("SYNTHETIC_SECRET")
    );
}

#[test]
fn own_likes_bounded_success_and_empty_collection_remain_incomplete() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for (body, expected_posts, expected_cursor) in [
        (timeline(), 1, Some("opaque + / = next")),
        (
            json!({"data":{"user":{"result":{"__typename":"User","timeline":{"timeline":{"instructions":[{"type":"TimelineAddEntries","entries":[]}]}}}}}}),
            0,
            None,
        ),
    ] {
        let mock = Mock { status: 200, body };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let output = pagination::collect(
            Output::new("graphql", Some("123".into())),
            1,
            None,
            |cursor| graph.page(Operation::Likes, "123", 5, cursor),
        )
        .unwrap();
        assert_eq!(output.posts.len(), expected_posts);
        assert_eq!(output.pages, 1);
        assert_eq!(output.next_cursor.as_deref(), expected_cursor);
        assert!(!output.complete && !output.request_failed);
    }
}

#[test]
fn own_likes_reject_wrong_user_discriminator_and_missing_root() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for typename in [Value::Null, json!("UserUnavailable"), json!("Unexpected")] {
        let mut body = timeline();
        body["data"]["user"]["result"]["__typename"] = typename;
        let mock = Mock { status: 200, body };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        assert_eq!(
            graph
                .page(Operation::Likes, "123", 5, None)
                .err()
                .unwrap()
                .diagnostic,
            Some(Diagnostic::ResponseRoot)
        );
    }
    let mock = Mock {
        status: 200,
        body: json!({"data":{"user":{"result":{"__typename":"User"}}}}),
    };
    let graph = Graphql {
        transport: &mock,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    assert_eq!(
        graph
            .page(Operation::Likes, "123", 5, None)
            .err()
            .unwrap()
            .diagnostic,
        Some(Diagnostic::ResponseRoot)
    );
}

#[test]
fn own_likes_errors_are_redacted_not_empty_successes() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for (status, body, kind) in [
        (
            401,
            json!({"message":"SYNTHETIC_SECRET"}),
            Kind::Authentication,
        ),
        (403, json!({"message":"SYNTHETIC_SECRET"}), Kind::Permission),
        (
            404,
            json!({"message":"SYNTHETIC_SECRET"}),
            Kind::ProtocolChanged,
        ),
        (429, json!({"message":"SYNTHETIC_SECRET"}), Kind::RateLimit),
        (503, json!({"message":"SYNTHETIC_SECRET"}), Kind::Network),
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
        let error = graph.page(Operation::Likes, "123", 5, None).err().unwrap();
        assert_eq!(error.kind, kind);
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
}
