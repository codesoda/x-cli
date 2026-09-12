use super::*;
mod bookmarks;
mod likes;
mod list_inventory;
mod list_members;
mod list_metadata;
mod lists;
mod relationships;
struct Mock {
    status: u16,
    body: Value,
}
impl Transport for Mock {
    fn get(&self, request: Request) -> Result<crate::transport::Response> {
        assert!(request.url.starts_with("https://x.com/i/api/graphql/"));
        assert!(request.headers.iter().any(|(name, _)| name == "cookie"));
        assert!(
            request
                .headers
                .iter()
                .any(|(name, _)| name == "x-csrf-token")
        );
        Ok(crate::transport::Response {
            status: self.status,
            body: self.body.to_string().into_bytes(),
            retry_after: Some(42),
        })
    }
}
#[test]
fn injected_identity_query_and_http_failures() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let mock = Mock {
        status: 200,
        body: json!({"data":{"viewer":{"user_results":{"result":{"__typename":"User","rest_id":"123","core":{"screen_name":"fixture"}}}}}}),
    };
    let graph = Graphql {
        transport: &mock,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    let actual = graph.identity().unwrap();
    assert_eq!(actual.id, "123");
    assert!(
        crate::config::verify_identity(
            &Identity {
                id: "456".into(),
                handle: "fixture".into()
            },
            &actual
        )
        .is_err()
    );
    for (status, kind) in [
        (401, Kind::Authentication),
        (403, Kind::Permission),
        (404, Kind::ProtocolChanged),
        (429, Kind::RateLimit),
    ] {
        let mock = Mock {
            status,
            body: json!({"message":"SYNTHETIC_SECRET"}),
        };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let error = graph.identity().unwrap_err();
        assert_eq!(error.kind, kind);
        assert!(!format!("{error:?}").contains("SYNTHETIC_SECRET"));
    }
}
#[test]
fn search_request_matches_reviewed_get_contract() {
    struct Search;
    impl Transport for Search {
        fn get(&self, request: Request) -> Result<crate::transport::Response> {
            let url = url::Url::parse(&request.url).unwrap();
            assert_eq!(url.host_str(), Some("x.com"));
            assert_eq!(
                url.path(),
                "/i/api/graphql/KPSo2_UWdOMpPJwjhfT1Qg/SearchTimeline"
            );
            let content_type = request
                .headers
                .iter()
                .find(|(name, _)| name == "content-type");
            assert!(content_type.is_some_and(|(_, value)| value.as_str() == "application/json"));
            let parameters: std::collections::HashMap<_, _> = url.query_pairs().collect();
            let variables: Value = serde_json::from_str(&parameters["variables"]).unwrap();
            assert_eq!(
                variables,
                json!({"rawQuery":"from:jack","count":5,"querySource":"typed_query","product":"Latest","withGrokTranslatedBio":false,"withQuickPromoteEligibilityTweetFields":false})
            );
            assert!(parameters.contains_key("features"));
            Ok(crate::transport::Response { status:200, retry_after:None,
                body:serde_json::to_vec(&json!({"data":{"search_by_raw_query":{"search_timeline":{"timeline":{"instructions":[{"type":"TimelineAddEntries","entries":[]}]}}}}})).unwrap() })
        }
    }
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    let graph = Graphql {
        transport: &Search,
        session: &session,
        bearer: Zeroizing::new("synthetic-public-token".into()),
    };
    assert!(
        graph
            .page(Operation::Search, "from:jack", 5, None)
            .unwrap()
            .posts
            .is_empty()
    );
}
#[test]
fn query_failures_identify_http_errors_and_missing_roots() {
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    for (status, body, expected) in [
        (
            400,
            json!({"message":"SYNTHETIC_SECRET"}),
            Diagnostic::Http { status: 400 },
        ),
        (
            200,
            json!({"errors":[{"code":999,"message":"SYNTHETIC_SECRET"}]}),
            Diagnostic::GraphqlErrors { code: Some(999) },
        ),
        (
            200,
            json!({"data":{"unexpected":"SYNTHETIC_SECRET"}}),
            Diagnostic::ResponseRoot,
        ),
    ] {
        let mock = Mock { status, body };
        let graph = Graphql {
            transport: &mock,
            session: &session,
            bearer: Zeroizing::new("synthetic-public-token".into()),
        };
        let error = graph
            .page(Operation::Search, "fixture", 5, None)
            .err()
            .expect("Expected query failure");
        assert_eq!(error.diagnostic, Some(expected));
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
}
#[test]
fn malformed_timeline_stages_are_distinct_and_redacted() {
    for (input, expected) in [
        (json!({}), Diagnostic::TimelineInstructions),
        (
            json!([{"type":"SYNTHETIC_SECRET"}]),
            Diagnostic::TimelineInstruction,
        ),
        (
            json!([{"type":"TimelineAddEntries","entries":[{"content":{}}]}]),
            Diagnostic::TimelineEntry,
        ),
        (
            json!([{"type":"TimelineAddEntries","entries":[{"content":{"itemContent":{"itemType":"SYNTHETIC_SECRET"}}}]}]),
            Diagnostic::TimelineItem,
        ),
        (
            json!([{"type":"TimelineAddEntries","entries":[{"content":{"cursorType":"Bottom"}}]}]),
            Diagnostic::Cursor,
        ),
    ] {
        let error = parse_page(&input).err().expect("Expected parsing failure");
        assert_eq!(error.diagnostic, Some(expected));
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("SYNTHETIC_SECRET")
        );
    }
    assert_eq!(
        parse_post(&json!({})).unwrap_err().diagnostic,
        Some(Diagnostic::Post)
    );
    assert_eq!(
        parse_identity(&json!({})).unwrap_err().diagnostic,
        Some(Diagnostic::Identity)
    );
}
fn tweet() -> Value {
    json!({"__typename":"Tweet","rest_id":"20","core":{"user_results":{"result":{"__typename":"User","rest_id":"12","core":{"screen_name":"jack"}}}},"legacy":{"full_text":"short"},"note_tweet":{"note_tweet_results":{"result":{"text":"long"}}}})
}
#[test]
fn post_wrapped_note() {
    let p =
        parse_post(&json!({"__typename":"TweetWithVisibilityResults","tweet":tweet()})).unwrap();
    assert_eq!(p.text, "long");
    assert_eq!(p.id, "20");
}
#[test]
fn errors_redacted() {
    for (code, kind) in [
        (89, Kind::Authentication),
        (88, Kind::RateLimit),
        (179, Kind::Permission),
        (999, Kind::ProtocolChanged),
    ] {
        let e =
            check_errors(&json!({"data":{},"errors":[{"code":code,"message":"cookie=SECRET"}]}))
                .unwrap_err();
        assert_eq!(e.kind, kind);
        assert!(!format!("{e:?}").contains("SECRET"));
    }
}
#[test]
fn page_module_replace() {
    let p=parse_page(&json!([{"type":"TimelineAddEntries","entries":[{"content":{"items":[{"item":{"itemContent":{"tweet_results":{"result":tweet()}}}}]}}]},{"type":"TimelineReplaceEntry","entry":{"content":{"cursorType":"Bottom","value":"next"}}}])).unwrap();
    assert_eq!(p.posts.len(), 1);
    assert_eq!(p.next_cursor.as_deref(), Some("next"));
}
#[test]
fn missing_roots_not_empty() {
    assert!(parse_page(&json!({})).is_err());
    assert!(parse_page(&json!([])).is_err());
    assert!(parse_page(&json!([{"type":"NewUnknownInstruction"}])).is_err());
    assert!(parse_page(&json!([{"type":"TimelineAddEntries","entries":[]}])).is_ok());
}
#[test]
fn tombstone_partial() {
    let p=parse_page(&json!([{"type":"TimelineAddEntries","entries":[{"content":{"itemContent":{"tweet_results":{"result":{"__typename":"TweetUnavailable"}}}}}]}])).unwrap();
    assert_eq!(p.warnings.len(), 1);
}
