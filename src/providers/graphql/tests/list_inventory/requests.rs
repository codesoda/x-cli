use super::*;
use crate::{pagination, transport::Response};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap, VecDeque},
};

struct Inventory {
    responses: RefCell<VecDeque<Response>>,
    cursors: RefCell<VecDeque<Option<String>>>,
}
impl Transport for Inventory {
    fn get(&self, request: Request) -> Result<Response> {
        let url = url::Url::parse(&request.url).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("x.com"));
        assert_eq!(
            url.path(),
            "/i/api/graphql/XU4wZWEjElhe--6doqFolA/ListsManagementPageTimeline"
        );
        let pairs: HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(pairs.len(), 2);
        assert!(!pairs.contains_key("fieldToggles"));
        let mut expected = json!({"count":5});
        if let Some(cursor) = self
            .cursors
            .borrow_mut()
            .pop_front()
            .expect("unexpected request")
        {
            expected["cursor"] = json!(cursor);
        }
        assert_eq!(
            serde_json::from_str::<Value>(&pairs["variables"]).unwrap(),
            expected
        );
        let features: Value = serde_json::from_str(&pairs["features"]).unwrap();
        let expected: BTreeSet<_> = include_str!("../../../operations/bookmark-features.txt")
            .lines()
            .collect();
        let actual: BTreeSet<_> = features
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 39);
        assert_eq!(features, Operation::Detail.features());
        assert!(request.headers.iter().any(|(name, value)| name == "content-type" && value.as_str() == "application/json"));
        Ok(self
            .responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected request"))
    }
}
fn response(status: u16, body: Value) -> Response {
    Response {
        status,
        body: serde_json::to_vec(&body).unwrap(),
        retry_after: Some(42),
    }
}
fn timeline(section: &str, cursor: Option<&str>) -> Value {
    let mut entries = vec![module(section, raw())];
    if let Some(cursor) = cursor {
        entries.push(json!({"content":{"cursorType":"Bottom","value":cursor}}));
    }
    body(instructions(entries))
}

#[test]
fn list_inventory_get_contract_pagination_merges_overlapping_sections() {
    let cursor = "opaque + / =";
    let transport = Inventory {
        responses: RefCell::new(
            [
                response(200, timeline("pinnedListModule", Some(cursor))),
                response(200, timeline("ownedSubscribedListModule", None)),
            ]
            .into(),
        ),
        cursors: RefCell::new([None, Some(cursor.into())].into()),
    };
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let graph = Graphql {
        transport: &transport,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    let out = pagination::collect_lists(
        Output::new("graphql", Some("123".into())),
        2,
        None,
        |cursor| graph.lists_page("123", 5, cursor),
    )
    .unwrap();
    assert_eq!(out.pages, 2);
    assert!(!out.complete && !out.request_failed);
    assert_eq!(out.stop_reason, "cursor_exhausted");
    assert_eq!(out.provenance.account_id.as_deref(), Some("123"));
    assert!(out.posts.is_empty() && out.users.is_none());
    let lists = out.lists.unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].management_sections, vec![Pinned, OwnedSubscribed]);
    assert!(transport.responses.borrow().is_empty());
}

#[test]
fn list_inventory_wrong_roots_and_failures_are_redacted() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for (status, value, kind) in [
        (
            200,
            json!({"data":{"user":{"result":{"timeline":{"timeline":{"instructions":[]}}}}}}),
            Kind::ProtocolChanged,
        ),
        (
            200,
            json!({"data":{"list":{"instructions":[]}}}),
            Kind::ProtocolChanged,
        ),
        (
            200,
            json!({"data":{"viewer":{"managementListsPageTimeline":{"edges":[]}}}}),
            Kind::ProtocolChanged,
        ),
        (200, body(Value::Null), Kind::ProtocolChanged),
        (
            200,
            body(json!([{"type":"SYNTHETIC_SECRET"}])),
            Kind::ProtocolChanged,
        ),
        (
            200,
            json!({"errors":[{"code":89,"message":"SYNTHETIC_SECRET"}]}),
            Kind::Authentication,
        ),
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
    ] {
        let mock = Mock {
            status,
            body: value,
        };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let error = graph.lists_page("123", 5, None).err().unwrap();
        assert_eq!(error.kind, kind);
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
}

#[test]
fn list_inventory_later_rate_limit_retains_data_and_never_retries() {
    let transport = Inventory {
        responses: RefCell::new(
            [
                response(200, timeline("pinned-list-module", Some("next"))),
                response(429, json!({"secret":"SYNTHETIC_SECRET"})),
            ]
            .into(),
        ),
        cursors: RefCell::new([None, Some("next".into())].into()),
    };
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let graph = Graphql {
        transport: &transport,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    let out = pagination::collect_lists(
        Output::new("graphql", Some("123".into())),
        3,
        None,
        |cursor| graph.lists_page("123", 5, cursor),
    )
    .unwrap();
    assert!(out.request_failed && !out.complete);
    assert_eq!(out.pages, 1);
    assert_eq!(out.stop_reason, "request_failed");
    assert_eq!(out.next_cursor.as_deref(), Some("next"));
    assert_eq!(out.lists.as_ref().unwrap().len(), 1);
    assert!(out.warnings.iter().any(|warning| warning.contains("42")));
    assert!(
        !serde_json::to_string(&out)
            .unwrap()
            .contains("SYNTHETIC_SECRET")
    );
    assert!(transport.responses.borrow().is_empty());
}
