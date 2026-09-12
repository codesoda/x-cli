use super::operations::{BUNDLE_SHA256, BUNDLE_URL, Operation};
use crate::{
    credentials::Session,
    error::{Diagnostic, Error, Kind, Result, protocol},
    model::{Identity, Output},
    pagination::Page,
    transport::{Request, Transport, check},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

mod parsing;
mod users;
pub use parsing::{check_errors, parse_identity, parse_page, parse_post};

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
            .append_pair("features", &op.features().to_string());
        if !matches!(
            op,
            Operation::Following
                | Operation::Followers
                | Operation::Bookmarks
                | Operation::ListPosts
        ) {
            url.query_pairs_mut()
                .append_pair("fieldToggles", &op.toggles().to_string());
        }
        let mut headers = self.session.headers();
        headers.extend([
            (
                "content-type".into(),
                Zeroizing::new("application/json".into()),
            ),
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
            )
            .at(Diagnostic::Http { status: r.status }));
        }
        check(&r, true).map_err(|e| e.at(Diagnostic::Http { status: r.status }))?;
        let v: Value =
            serde_json::from_slice(&r.body).map_err(|_| protocol().at(Diagnostic::Json))?;
        check_errors(&v)?;
        if matches!(
            op,
            Operation::Likes | Operation::Following | Operation::Followers
        ) && v
            .pointer("/data/user/result/__typename")
            .and_then(Value::as_str)
            != Some("User")
        {
            return Err(protocol().at(Diagnostic::ResponseRoot));
        }
        v.pointer(op.root())
            .cloned()
            .ok_or_else(|| protocol().at(Diagnostic::ResponseRoot))
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
    pub fn users_page(
        &self,
        op: Operation,
        account_id: &str,
        count: u32,
        cursor: Option<&str>,
    ) -> Result<crate::pagination::UsersPage> {
        if !matches!(op, Operation::Following | Operation::Followers) {
            return Err(Error::new(
                Kind::Unsupported,
                "This query is not a user collection",
            ));
        }
        crate::input::id(account_id)?;
        let mut variables = json!({"userId":account_id,"count":count,
            "includePromotedContent":false,"withGrokTranslatedBio":false});
        if let Some(cursor) = cursor {
            if cursor.is_empty() || cursor.len() > 4096 {
                return Err(Error::new(
                    Kind::InvalidInput,
                    "Cursor must contain 1 to 4096 bytes",
                ));
            }
            variables["cursor"] = json!(cursor);
        }
        users::parse_users_page(&self.query(op, variables)?)
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
            Operation::Bookmarks => json!({"count":count,"includePromotedContent":true}),
            Operation::ListPosts => json!({"listId":target,"count":count}),
            Operation::Likes => {
                json!({"userId":target,"count":count,"includePromotedContent":false,"withClientEventToken":false,"withBirdwatchNotes":false,"withVoice":false})
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

#[cfg(test)]
mod tests;
