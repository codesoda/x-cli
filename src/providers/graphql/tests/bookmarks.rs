use super::*;
use crate::{pagination, transport::Response};
use std::cell::RefCell;

struct Bookmarks {
    responses: RefCell<std::collections::VecDeque<Response>>,
    cursors: RefCell<Vec<Option<String>>>,
}
impl Transport for Bookmarks {
    fn get(&self, request: Request) -> Result<Response> {
        let url = url::Url::parse(&request.url).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("x.com"));
        assert_eq!(
            url.path(),
            "/i/api/graphql/tF6KOjmZM0WGcB2Q0mfwhw/Bookmarks"
        );
        assert!(request.headers.iter().any(|(name, value)|
            name == "content-type" && value.as_str() == "application/json"));
        let parameters: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let variables: Value = serde_json::from_str(&parameters["variables"]).unwrap();
        let cursor = self.cursors.borrow_mut().remove(0);
        let mut expected = json!({"count":5,"includePromotedContent":true});
        if let Some(cursor) = cursor {
            expected["cursor"] = json!(cursor);
        }
        assert_eq!(variables, expected);
        let features: Value = serde_json::from_str(&parameters["features"]).unwrap();
        assert_eq!(features, Operation::Detail.features());
        assert_eq!(features.as_object().unwrap().len(), 39);
        assert!(!parameters.contains_key("fieldToggles"));
        Ok(self
            .responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected extra request"))
    }
}
fn response(status: u16, body: Value) -> Response {
    Response {
        status,
        body: serde_json::to_vec(&body).unwrap(),
        retry_after: Some(42),
    }
}
fn timeline(cursor: Option<&str>) -> Value {
    let mut entries = vec![json!({"content":{"itemContent":{"tweet_results":{"result":tweet()}}}})];
    if let Some(cursor) = cursor {
        entries.push(json!({"content":{"cursorType":"Bottom","value":cursor}}));
    }
    json!({"data":{"bookmark_timeline_v2":{"timeline":{"instructions":[{"type":"TimelineAddEntries","entries":entries}]}}}})
}

#[test]
fn bookmark_query_parses_posts_and_opaque_cursors() {
    let cursor = "opaque + / = continuation";
    let mock = Bookmarks {
        responses: RefCell::new(
            [
                response(200, timeline(Some(cursor))),
                response(200, timeline(None)),
            ]
            .into(),
        ),
        cursors: RefCell::new(vec![None, Some(cursor.into())]),
    };
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let graph = Graphql {
        transport: &mock,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    let output = pagination::collect(
        Output::new("graphql", Some("123".into())),
        2,
        None,
        |cursor| graph.page(Operation::Bookmarks, "", 5, cursor),
    )
    .unwrap();
    assert_eq!(output.posts.len(), 1); // Duplicate post across pages is deduplicated.
    assert_eq!(output.posts[0].id, "20");
    assert_eq!(output.posts[0].text, "long");
    assert_eq!(output.pages, 2);
    assert!(!output.complete && !output.request_failed);
    assert!(output.next_cursor.is_none());
    assert_eq!(output.provenance.account_id.as_deref(), Some("123"));
    assert!(mock.responses.borrow().is_empty());
}

#[test]
fn bookmark_failures_are_not_empty_successes_or_raw_payloads() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for (status, body, kind) in [
        (
            401,
            json!({"secret":"SYNTHETIC_SECRET"}),
            Kind::Authentication,
        ),
        (403, json!({"secret":"SYNTHETIC_SECRET"}), Kind::Permission),
        (
            404,
            json!({"secret":"SYNTHETIC_SECRET"}),
            Kind::ProtocolChanged,
        ),
        (429, json!({"secret":"SYNTHETIC_SECRET"}), Kind::RateLimit),
        (503, json!({"secret":"SYNTHETIC_SECRET"}), Kind::Network),
        (
            200,
            json!({"data":{"wrong_root":"SYNTHETIC_SECRET"}}),
            Kind::ProtocolChanged,
        ),
        (
            200,
            json!({"errors":[{"code":89,"message":"SYNTHETIC_SECRET"}]}),
            Kind::Authentication,
        ),
        (
            200,
            json!({"data":{"bookmark_timeline_v2":{"timeline":{"instructions":[{"type":"SYNTHETIC_SECRET"}]}}}}),
            Kind::ProtocolChanged,
        ),
    ] {
        let mock = Mock { status, body };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let error = graph.page(Operation::Bookmarks, "", 5, None).err().unwrap();
        assert_eq!(error.kind, kind);
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
}

#[test]
fn bookmark_later_failure_preserves_partial_data_without_retry() {
    let mock = Bookmarks {
        responses: RefCell::new(
            [
                response(200, timeline(Some("next"))),
                response(429, json!({"secret":"SYNTHETIC_SECRET"})),
            ]
            .into(),
        ),
        cursors: RefCell::new(vec![None, Some("next".into())]),
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
        |cursor| graph.page(Operation::Bookmarks, "", 5, cursor),
    )
    .unwrap();
    assert_eq!(output.posts.len(), 1);
    assert!(output.request_failed && !output.complete);
    assert_eq!(output.pages, 1);
    assert!(mock.responses.borrow().is_empty());
    assert!(
        !serde_json::to_string(&output)
            .unwrap()
            .contains("SYNTHETIC_SECRET")
    );
}
