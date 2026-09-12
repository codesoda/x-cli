use super::*;
use crate::{model::ListVisibility, transport::Response};
use std::cell::Cell;

fn metadata() -> Value {
    json!({"id_str":"456","name":"Synthetic list","description":"Fixture description",
        "mode":"Private","user_results":{"result":{"__typename":"User",
        "rest_id":"789","core":{"screen_name":"owner_fixture"}}}})
}

#[test]
fn list_metadata_get_contract_and_singleton_output() {
    struct Metadata(Cell<u32>);
    impl Transport for Metadata {
        fn get(&self, request: Request) -> Result<Response> {
            self.0.set(self.0.get() + 1);
            let url = url::Url::parse(&request.url).unwrap();
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some("x.com"));
            assert_eq!(
                url.path(),
                "/i/api/graphql/EAARFZGlY-JHdLJbKZAA5g/ListByRestId"
            );
            let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
            assert_eq!(pairs.len(), 2);
            assert!(!pairs.contains_key("fieldToggles"));
            assert_eq!(
                serde_json::from_str::<Value>(&pairs["variables"]).unwrap(),
                json!({"listId":"456"})
            );
            // Independent exact five-name fixture, including the established value policy.
            assert_eq!(
                serde_json::from_str::<Value>(&pairs["features"]).unwrap(),
                json!({
                    "profile_label_improvements_pcf_label_in_post_enabled":true,
                    "responsive_web_profile_redirect_enabled":true,
                    "rweb_tipjar_consumption_enabled":false,
                    "verified_phone_label_enabled":false,
                    "responsive_web_graphql_timeline_navigation_enabled":true
                })
            );
            assert!(request.headers.iter().any(
                |(name, value)| name == "content-type" && value.as_str() == "application/json"
            ));
            Ok(Response {
                status: 200,
                retry_after: None,
                body: serde_json::to_vec(&json!({"data":{"list":metadata()}})).unwrap(),
            })
        }
    }
    let transport = Metadata(Cell::new(0));
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let graph = Graphql {
        transport: &transport,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    let output = graph.list_metadata("456", "123").unwrap();
    assert_eq!(transport.0.get(), 1);
    assert!(output.complete && !output.request_failed);
    assert_eq!(output.pages, 1);
    assert_eq!(output.stop_reason, "single_list");
    assert!(output.next_cursor.is_none());
    assert!(output.posts.is_empty() && output.users.is_none());
    assert_eq!(output.provenance.account_id.as_deref(), Some("123"));
    let lists = output.lists.unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].id, "456");
    assert_eq!(lists[0].owner.as_ref().unwrap().id, "789");
    assert_eq!(lists[0].visibility, Some(ListVisibility::Private));
    assert_eq!(lists[0].url, "https://x.com/i/lists/456");
}

#[test]
fn list_metadata_optional_fields_and_case_normalized_mode() {
    let parse = super::super::list_metadata::parse;
    let minimal = json!({"id_str":"456","name":"Fixture"});
    let result = parse(&minimal).unwrap();
    assert!(result.owner.is_none() && result.description.is_none() && result.visibility.is_none());
    let mut value = minimal;
    for mode in [Value::Null, json!("PUBLIC"), json!("pRiVaTe")] {
        value["description"] = Value::Null;
        value["mode"] = mode.clone();
        value["user_results"] = json!({"result":{"__typename":"UserUnavailable"}});
        let result = parse(&value).unwrap();
        assert!(result.owner.is_none() && result.description.is_none());
        assert_eq!(
            result.visibility,
            match mode.as_str() {
                Some("PUBLIC") => Some(ListVisibility::Public),
                Some(_) => Some(ListVisibility::Private),
                None => None,
            }
        );
    }
    value["description"] = json!("");
    value["user_results"] = json!({"result":null});
    assert_eq!(parse(&value).unwrap().description.as_deref(), Some(""));
    // Neither a legacy owner nor an unrelated rest_id overrides the reviewed fields.
    value["user"] = json!({"id_str":"999","screen_name":"legacy"});
    value["rest_id"] = json!("999");
    let result = parse(&value).unwrap();
    assert_eq!(result.id, "456");
    assert!(result.owner.is_none());
}

#[test]
fn list_metadata_malformed_fields_fail_without_invented_visibility_or_owner() {
    let parse = super::super::list_metadata::parse;
    for (field, invalid) in [
        ("id_str", Value::Null),
        ("id_str", json!(456)),
        ("id_str", json!("01")),
        ("id_str", json!("0")),
        ("name", Value::Null),
        ("name", json!(4)),
        ("description", json!(false)),
        ("mode", json!("unknown")),
        ("mode", json!(" public ")),
        ("mode", json!(false)),
        ("mode", json!({})),
    ] {
        let mut value = metadata();
        value[field] = invalid;
        assert_eq!(parse(&value).unwrap_err().kind, Kind::ProtocolChanged);
    }
    let mut value = metadata();
    value.as_object_mut().unwrap().remove("id_str");
    value["rest_id"] = json!("456");
    assert_eq!(parse(&value).unwrap_err().kind, Kind::ProtocolChanged);
    for owner in [
        json!({"__typename":"User"}),
        json!({"__typename":"User","rest_id":789,"core":{"screen_name":"fixture"}}),
        json!({"__typename":"User","rest_id":"0","core":{"screen_name":"fixture"}}),
        json!({"__typename":"User","rest_id":"789","legacy":{"screen_name":"fixture"}}),
        json!({"__typename":"User","rest_id":"789","core":{"screen_name":"bad/handle"}}),
        json!({"__typename":"User","rest_id":"789","core":{"screen_name":null},"legacy":{"screen_name":"fixture"}}),
    ] {
        let mut value = metadata();
        value["user_results"] = json!({"result":owner});
        assert_eq!(parse(&value).unwrap_err().kind, Kind::ProtocolChanged);
    }
}

#[test]
fn list_metadata_errors_are_redacted_and_never_successful_empty_lists() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for (status, body, kind) in [
        (200, json!({"data":{}}), Kind::ProtocolChanged),
        (200, json!({"data":{"list":null}}), Kind::ProtocolChanged),
        (200, json!({"data":{"list":[]}}), Kind::ProtocolChanged),
        (
            200,
            json!({"data":{"list":{"id_str":"457","name":"SYNTHETIC_SECRET"}}}),
            Kind::ProtocolChanged,
        ),
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
            json!({"data":{"list":metadata()},"errors":[{"code":999,"message":"SYNTHETIC_SECRET"}]}),
            Kind::ProtocolChanged,
        ),
    ] {
        let mock = Mock { status, body };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let error = graph.list_metadata("456", "123").unwrap_err();
        assert_eq!(error.kind, kind);
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
}
