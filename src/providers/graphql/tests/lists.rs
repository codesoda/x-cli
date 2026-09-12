use super::*;
use crate::{pagination, transport::Response};
use std::cell::Cell;

fn timeline() -> Value {
    // No List typename is required by the reviewed call site. Exercise module
    // posts and replacement cursors, not only flat TimelineAddEntries.
    json!({"data":{"list":{"tweets_timeline":{"timeline":{"instructions":[
        {"type":"TimelineAddEntries","entries":[{"content":{"items":[
            {"item":{"itemContent":{"tweet_results":{"result":tweet()}}}}
        ]}}]},
        {"type":"TimelineReplaceEntry","entry":{"content":{"cursorType":"Bottom","value":"opaque + / = next"}}}
    ]}}}}})
}
struct ListPosts {
    calls: Cell<u32>,
    fail_second: bool,
}
impl Transport for ListPosts {
    fn get(&self, request: Request) -> Result<Response> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        assert!(call < 2, "unexpected repeated request");
        let url = url::Url::parse(&request.url).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("x.com"));
        assert_eq!(
            url.path(),
            "/i/api/graphql/u6PUF1835XGBkf6MQZUV8A/ListLatestTweetsTimeline"
        );
        let parameters: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let variables: Value = serde_json::from_str(&parameters["variables"]).unwrap();
        let mut expected = json!({"listId":"456","count":5});
        if call > 0 {
            expected["cursor"] = json!("opaque + / = next");
        }
        assert_eq!(variables, expected); // No user ID, ranked/promoted/voice flags.
        let toggles: Value = serde_json::from_str(&parameters["fieldToggles"]).unwrap();
        assert_eq!(toggles, json!({}));
        let features: Value = serde_json::from_str(&parameters["features"]).unwrap();
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
        assert!(request.headers.iter().any(|(name,value)| name=="content-type" && value.as_str()=="application/json"));
        let failed = call > 0 && self.fail_second;
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
fn list_posts_request_modules_and_repeated_cursor() {
    let mock = ListPosts {
        calls: Cell::new(0),
        fail_second: false,
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
        |cursor| graph.page(Operation::ListPosts, "456", 5, cursor),
    )
    .unwrap();
    assert_eq!(output.posts.len(), 1);
    assert_eq!(output.posts[0].text, "long");
    assert_eq!(output.pages, 2);
    assert_eq!(output.stop_reason, "repeated_cursor");
    assert!(!output.complete && !output.request_failed);
    assert_eq!(output.provenance.account_id.as_deref(), Some("123"));
}

#[test]
fn list_posts_later_failure_preserves_data_without_retry() {
    let mock = ListPosts {
        calls: Cell::new(0),
        fail_second: true,
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
        |cursor| graph.page(Operation::ListPosts, "456", 5, cursor),
    )
    .unwrap();
    assert_eq!(output.posts.len(), 1);
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
fn list_posts_empty_collection_is_valid_but_not_complete() {
    let mock = Mock {
        status: 200,
        body: json!({"data":{"list":{"tweets_timeline":{"timeline":{"instructions":[{"type":"TimelineAddEntries","entries":[]}]}}}}}),
    };
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let graph = Graphql {
        transport: &mock,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    let output = pagination::collect(
        Output::new("graphql", Some("123".into())),
        1,
        None,
        |cursor| graph.page(Operation::ListPosts, "456", 5, cursor),
    )
    .unwrap();
    assert!(output.posts.is_empty() && !output.complete && !output.request_failed);
    assert_eq!(output.stop_reason, "cursor_exhausted");
}

#[test]
fn list_posts_reject_missing_roots_preview_and_upstream_failures() {
    let mut preview = timeline();
    preview["data"]["list"]["tweets_timeline"]["timeline"]["instructions"][0]["entries"][0]["content"]
        ["items"][0]["item"]["itemContent"]["tweet_results"]["result"] =
        json!({"__typename":"TweetPreviewDisplay"});
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for (status, body, kind) in [
        (200, json!({"data":{"list":null}}), Kind::ProtocolChanged),
        (200, preview, Kind::ProtocolChanged),
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
        let error = graph
            .page(Operation::ListPosts, "456", 5, None)
            .err()
            .unwrap();
        assert_eq!(error.kind, kind);
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
}
