use super::operations::{BUNDLE_SHA256, BUNDLE_URL, Operation};
use crate::{
    credentials::Session,
    error::{Error, Kind, Result, protocol},
    model::{Identity, Output, Post},
    pagination::Page,
    transport::{Request, Transport, check},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub struct Graphql<'a> {
    transport: &'a dyn Transport,
    session: &'a Session,
    bearer: Zeroizing<String>,
}
impl<'a> Graphql<'a> {
    /// Fetch a pinned public web-client asset WITHOUT credentials. Never evaluate JavaScript.
    pub fn new(transport: &'a dyn Transport, session: &'a Session) -> Result<Self> {
        let response = transport.get(Request::public(BUNDLE_URL.into()))?;
        check(&response, false)?;
        if format!("{:x}", Sha256::digest(&response.body)) != BUNDLE_SHA256 {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "Pinned X web-client asset changed; review operation manifest before continuing",
            ));
        }
        let text = std::str::from_utf8(&response.body).map_err(|_| protocol())?;
        let candidates: std::collections::HashSet<_> = text
            .split(['\"', '\''])
            .filter(|s| {
                s.starts_with("AAAAAA")
                    && s.len() >= 80
                    && s.len() <= 256
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'%' | b'_' | b'-'))
            })
            .collect();
        if candidates.len() != 1 {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "Cannot identify the pinned public web-client authorization value",
            ));
        }
        Ok(Self {
            transport,
            session,
            bearer: Zeroizing::new(candidates.into_iter().next().expect("one candidate").into()),
        })
    }
    fn query(&self, op: Operation, variables: Value) -> Result<Value> {
        let mut url = url::Url::parse(&format!(
            "https://x.com/i/api/graphql/{}/{}",
            op.id(),
            op.name()
        ))
        .map_err(|_| protocol())?;
        url.query_pairs_mut()
            .append_pair("variables", &variables.to_string())
            .append_pair("features", &op.features().to_string())
            .append_pair("fieldToggles", &op.toggles().to_string());
        let mut headers = self.session.headers();
        headers.extend([
            (
                "authorization".into(),
                Zeroizing::new(format!("Bearer {}", self.bearer.as_str())),
            ),
            (
                "x-twitter-auth-type".into(),
                Zeroizing::new("OAuth2Session".into()),
            ),
            ("x-twitter-active-user".into(), Zeroizing::new("yes".into())),
            (
                "x-twitter-client-language".into(),
                Zeroizing::new("en".into()),
            ),
            ("referer".into(), Zeroizing::new("https://x.com/".into())),
        ]);
        let r = self.transport.get(Request {
            url: url.into(),
            headers,
        })?;
        // A removed persisted-query URL is protocol drift, not evidence a post was deleted.
        if r.status == 404 {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "X query endpoint unavailable; operation manifest may need review",
            ));
        }
        check(&r, true)?;
        let v: Value = serde_json::from_slice(&r.body).map_err(|_| protocol())?;
        check_errors(&v)?;
        v.pointer(op.root()).cloned().ok_or_else(protocol)
    }
    pub fn identity(&self) -> Result<Identity> {
        parse_identity(&self.query(
            Operation::Viewer,
            json!({"withCommunitiesMemberships":false}),
        )?)
    }
    pub fn user(&self, handle: &str) -> Result<Identity> {
        crate::input::handle(handle)?;
        parse_identity(&self.query(
            Operation::User,
            json!({"screen_name":handle,"withGrokTranslatedBio":false}),
        )?)
    }
    pub fn read(&self, id: &str, account: &str) -> Result<Output> {
        crate::input::id(id)?;
        let p=parse_post(&self.query(Operation::Post,json!({"tweetId":id,"withCommunity":false,"includePromotedContent":false,"withVoice":false}))?)?;
        if p.id != id {
            return Err(protocol());
        }
        let mut o = Output::new("graphql", Some(account.into()));
        o.posts.push(p);
        o.pages = 1;
        o.complete = true;
        o.stop_reason = "single_post".into();
        o.warnings.push("Unofficial source-verified protocol; authenticated rollout interoperability is not guaranteed".into());
        Ok(o)
    }
    pub fn page(
        &self,
        op: Operation,
        target: &str,
        count: u32,
        cursor: Option<&str>,
    ) -> Result<Page> {
        let mut v = match op {
            Operation::Detail => {
                json!({"focalTweetId":target,"rankingMode":"Relevance","includePromotedContent":false,"withCommunity":false,"withQuickPromoteEligibilityTweetFields":false,"withBirdwatchNotes":false,"withVoice":false})
            }
            Operation::Search => {
                json!({"rawQuery":target,"count":count,"querySource":"typed_query","product":"Latest","withGrokTranslatedBio":false,"withQuickPromoteEligibilityTweetFields":false})
            }
            Operation::Timeline => {
                json!({"userId":target,"count":count,"includePromotedContent":false,"withQuickPromoteEligibilityTweetFields":false,"withVoice":false})
            }
            _ => {
                return Err(Error::new(
                    Kind::Unsupported,
                    "This query does not support pagination",
                ));
            }
        };
        if let Some(c) = cursor {
            if c.len() > 4096 {
                return Err(Error::new(Kind::InvalidInput, "Cursor exceeds 4096 bytes"));
            }
            v["cursor"] = json!(c);
        }
        parse_page(&self.query(op, v)?)
    }
}
pub fn check_errors(v: &Value) -> Result<()> {
    if let Some(errors) = v.get("errors") {
        let errors = errors.as_array().ok_or_else(protocol)?;
        if !errors.is_empty() {
            // Never include upstream messages: they may echo request/session data.
            for e in errors {
                match e["code"].as_u64() {
                    Some(32 | 89 | 215 | 239) => {
                        return Err(Error::new(
                            Kind::Authentication,
                            "X rejected or expired the session",
                        ));
                    }
                    Some(88 | 420) => {
                        let mut e =
                            Error::new(Kind::RateLimit, "X rate limit reached; no automatic retry");
                        e.retry_after_seconds = Some(60);
                        return Err(e);
                    }
                    Some(63 | 64 | 179 | 200) => {
                        return Err(Error::new(Kind::Permission, "X denied access"));
                    }
                    Some(144) => {
                        return Err(Error::new(
                            Kind::Unavailable,
                            "X reports no available post; deletion is not established",
                        ));
                    }
                    _ => {}
                }
            }
            return Err(protocol());
        }
    }
    Ok(())
}
pub fn parse_identity(v: &Value) -> Result<Identity> {
    if v["__typename"] == "UserUnavailable" {
        return Err(Error::new(Kind::Unavailable, "X user is unavailable"));
    }
    if v["__typename"] != "User" {
        return Err(protocol());
    }
    let id = v["rest_id"].as_str().ok_or_else(protocol)?;
    let handle = v
        .pointer("/core/screen_name")
        .or_else(|| v.pointer("/legacy/screen_name"))
        .and_then(Value::as_str)
        .ok_or_else(protocol)?;
    crate::input::id(id).map_err(|_| protocol())?;
    crate::input::handle(handle).map_err(|_| protocol())?;
    Ok(Identity {
        id: id.into(),
        handle: handle.into(),
    })
}
pub fn parse_post(value: &Value) -> Result<Post> {
    let v = if value["__typename"] == "TweetWithVisibilityResults" {
        &value["tweet"]
    } else {
        value
    };
    if matches!(
        v["__typename"].as_str(),
        Some("TweetUnavailable" | "TweetTombstone")
    ) {
        return Err(Error::new(
            Kind::Unavailable,
            "X post unavailable; exact reason not established",
        ));
    }
    if v["__typename"] != "Tweet" {
        return Err(protocol());
    }
    let id = v["rest_id"].as_str().ok_or_else(protocol)?;
    crate::input::id(id).map_err(|_| protocol())?;
    let author = parse_identity(&v["core"]["user_results"]["result"])?;
    let legacy = v["legacy"].as_object().ok_or_else(protocol)?;
    let text = v
        .pointer("/note_tweet/note_tweet_results/result/text")
        .or_else(|| legacy.get("full_text"))
        .and_then(Value::as_str)
        .ok_or_else(protocol)?;
    let parent_id = match legacy.get("in_reply_to_status_id_str") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(crate::input::id(s).map_err(|_| protocol())?),
        _ => return Err(protocol()),
    };
    Ok(Post {
        id: id.into(),
        url: format!("https://x.com/{}/status/{id}", author.handle),
        author,
        text: text.into(),
        created_at: legacy
            .get("created_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
        parent_id,
        parent_known: true,
    })
}
pub fn parse_page(v: &Value) -> Result<Page> {
    let instructions = v.as_array().ok_or_else(protocol)?;
    let mut page = Page {
        posts: vec![],
        next_cursor: None,
        warnings: vec![],
    };
    let mut recognized = false;
    for instruction in instructions {
        match instruction["type"].as_str() {
            Some("TimelineAddEntries") => {
                recognized = true;
                for entry in instruction["entries"].as_array().ok_or_else(protocol)? {
                    entry_content(&entry["content"], &mut page)?;
                }
            }
            Some("TimelineReplaceEntry" | "TimelinePinEntry") => {
                recognized = true;
                entry_content(&instruction["entry"]["content"], &mut page)?;
            }
            Some("TimelineAddToModule") => {
                recognized = true;
                for item in instruction["moduleItems"].as_array().ok_or_else(protocol)? {
                    item_content(&item["item"]["itemContent"], &mut page)?;
                }
            }
            Some("TimelineTerminateTimeline" | "TimelineClearCache") => {
                recognized = true;
            }
            Some("TimelineShowAlert" | "TimelineShowCover") => page
                .warnings
                .push("Upstream supplied an alert or visibility cover".into()),
            _ => return Err(protocol()),
        }
    }
    if !recognized {
        return Err(protocol());
    }
    Ok(page)
}
fn entry_content(v: &Value, page: &mut Page) -> Result<()> {
    if let Some(cursor_type) = v["cursorType"].as_str() {
        let cursor = v["value"].as_str().ok_or_else(protocol)?;
        if cursor_type == "Bottom" {
            if page.next_cursor.as_deref().is_some_and(|old| old != cursor) {
                page.warnings
                    .push("Multiple bottom cursors; only the last continuation is exposed".into());
            }
            page.next_cursor = Some(cursor.into());
        } else if cursor_type != "Top" {
            page.warnings
                .push("Additional cursor branch not traversed".into());
        }
        return Ok(());
    }
    if let Some(items) = v.get("items") {
        for item in items.as_array().ok_or_else(protocol)? {
            item_content(&item["item"]["itemContent"], page)?;
        }
        return Ok(());
    }
    if let Some(item) = v.get("itemContent") {
        return item_content(item, page);
    }
    Err(protocol())
}
fn item_content(v: &Value, page: &mut Page) -> Result<()> {
    if v.get("promotedMetadata").is_some() {
        page.warnings.push("Promoted item omitted".into());
        return Ok(());
    }
    if let Some(result) = v.pointer("/tweet_results/result") {
        match parse_post(result) {
            Ok(p) => page.posts.push(p),
            Err(e) if e.kind == Kind::Unavailable => page.warnings.push(e.message.into()),
            Err(e) => return Err(e),
        }
    } else if v.get("cursorType").is_some() {
        entry_content(v, page)?;
    } else if matches!(
        v["itemType"].as_str(),
        Some(
            "TimelineTimelineUser"
                | "TimelineTombstone"
                | "TimelinePrompt"
                | "TimelineMessage"
                | "TimelineTimelineLabel"
        )
    ) {
        page.warnings
            .push("Non-post or unavailable timeline item omitted".into());
    } else {
        return Err(protocol());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
    /// Fetches only the pinned public asset, without session headers or X queries.
    #[test]
    #[ignore = "opt-in public web-client asset check; no authenticated requests"]
    fn live_public_manifest() {
        let transport = crate::transport::Http::new().unwrap();
        let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
        assert!(
            Graphql::new(&transport, &session).is_ok(),
            "Pinned public manifest unavailable or changed"
        );
    }
    fn tweet() -> Value {
        json!({"__typename":"Tweet","rest_id":"20","core":{"user_results":{"result":{"__typename":"User","rest_id":"12","core":{"screen_name":"jack"}}}},"legacy":{"full_text":"short"},"note_tweet":{"note_tweet_results":{"result":{"text":"long"}}}})
    }
    #[test]
    fn post_wrapped_note() {
        let p = parse_post(&json!({"__typename":"TweetWithVisibilityResults","tweet":tweet()}))
            .unwrap();
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
            let e = check_errors(
                &json!({"data":{},"errors":[{"code":code,"message":"cookie=SECRET"}]}),
            )
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
}
