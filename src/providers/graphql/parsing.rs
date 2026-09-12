//! Normalization and fail-closed parsing, independent of HTTP query construction.
use crate::{
    error::{Diagnostic, Error, Kind, Result, protocol},
    model::{Identity, Post},
    pagination::Page,
};
use serde_json::Value;

pub fn check_errors(v: &Value) -> Result<()> {
    let Some(errors) = v.get("errors") else {
        return Ok(());
    };
    let errors = errors
        .as_array()
        .ok_or_else(|| protocol().at(Diagnostic::GraphqlErrors { code: None }))?;
    for entry in errors {
        let code = entry["code"].as_u64();
        let error = match code {
            Some(32 | 89 | 215 | 239) => {
                Error::new(Kind::Authentication, "X rejected or expired the session")
            }
            Some(88 | 420) => {
                let mut error =
                    Error::new(Kind::RateLimit, "X rate limit reached; no automatic retry");
                error.retry_after_seconds = Some(60);
                error
            }
            Some(63 | 64 | 179 | 200) => Error::new(Kind::Permission, "X denied access"),
            Some(144) => Error::new(
                Kind::Unavailable,
                "X reports no available post; deletion is not established",
            ),
            _ => continue,
        };
        return Err(error.at(Diagnostic::GraphqlErrors {
            code: code.and_then(|c| u32::try_from(c).ok()),
        }));
    }
    if let Some(first) = errors.first() {
        let code = first["code"].as_u64().and_then(|c| u32::try_from(c).ok());
        return Err(protocol().at(Diagnostic::GraphqlErrors { code }));
    }
    Ok(())
}
pub fn parse_identity(v: &Value) -> Result<Identity> {
    (|| {
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
    })()
    .map_err(|e: Error| e.at(Diagnostic::Identity))
}
pub fn parse_post(value: &Value) -> Result<Post> {
    (|| {
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
    })()
    .map_err(|e: Error| e.at(Diagnostic::Post))
}
pub fn parse_page(v: &Value) -> Result<Page> {
    let instructions = v
        .as_array()
        .ok_or_else(|| protocol().at(Diagnostic::TimelineInstructions))?;
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
                for entry in instruction["entries"]
                    .as_array()
                    .ok_or_else(|| protocol().at(Diagnostic::TimelineInstructions))?
                {
                    entry_content(&entry["content"], &mut page)?;
                }
            }
            Some("TimelineReplaceEntry" | "TimelinePinEntry") => {
                recognized = true;
                entry_content(&instruction["entry"]["content"], &mut page)?;
            }
            Some("TimelineAddToModule") => {
                recognized = true;
                for item in instruction["moduleItems"]
                    .as_array()
                    .ok_or_else(|| protocol().at(Diagnostic::TimelineInstructions))?
                {
                    item_content(&item["item"]["itemContent"], &mut page)?;
                }
            }
            Some("TimelineTerminateTimeline" | "TimelineClearCache") => {
                recognized = true;
            }
            Some("TimelineShowAlert" | "TimelineShowCover") => page
                .warnings
                .push("Upstream supplied an alert or visibility cover".into()),
            _ => return Err(protocol().at(Diagnostic::TimelineInstruction)),
        }
    }
    if !recognized {
        return Err(protocol().at(Diagnostic::TimelineInstructions));
    }
    Ok(page)
}
fn entry_content(v: &Value, page: &mut Page) -> Result<()> {
    if let Some(cursor_type) = v["cursorType"].as_str() {
        let cursor = v["value"]
            .as_str()
            .ok_or_else(|| protocol().at(Diagnostic::Cursor))?;
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
        for item in items
            .as_array()
            .ok_or_else(|| protocol().at(Diagnostic::TimelineEntry))?
        {
            item_content(&item["item"]["itemContent"], page)?;
        }
        return Ok(());
    }
    if let Some(item) = v.get("itemContent") {
        return item_content(item, page);
    }
    Err(protocol().at(Diagnostic::TimelineEntry))
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
        return Err(protocol().at(Diagnostic::TimelineItem));
    }
    Ok(())
}
