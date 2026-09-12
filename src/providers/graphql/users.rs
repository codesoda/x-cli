//! Strict user-timeline projection. Unknown shapes are not empty relationship results.
use super::parse_identity;
use crate::{
    error::{Diagnostic, Result, protocol},
    pagination::UsersPage,
};
use serde_json::Value;

pub(super) fn parse_users_page(value: &Value) -> Result<UsersPage> {
    let instructions = value
        .as_array()
        .ok_or_else(|| protocol().at(Diagnostic::TimelineInstructions))?;
    let mut page = UsersPage {
        users: vec![],
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
                    content(&entry["content"], &mut page)?;
                }
            }
            Some("TimelineReplaceEntry" | "TimelinePinEntry") => {
                recognized = true;
                content(&instruction["entry"]["content"], &mut page)?;
            }
            Some("TimelineAddToModule") => {
                recognized = true;
                for entry in instruction["moduleItems"]
                    .as_array()
                    .ok_or_else(|| protocol().at(Diagnostic::TimelineInstructions))?
                {
                    item(&entry["item"]["itemContent"], &mut page)?;
                }
            }
            Some("TimelineTerminateTimeline" | "TimelineClearCache") => recognized = true,
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
fn cursor(value: &Value, page: &mut UsersPage) -> Result<()> {
    let kind = value["cursorType"]
        .as_str()
        .ok_or_else(|| protocol().at(Diagnostic::Cursor))?;
    let cursor = value["value"]
        .as_str()
        .ok_or_else(|| protocol().at(Diagnostic::Cursor))?;
    if kind == "Bottom" {
        if page
            .next_cursor
            .as_deref()
            .is_some_and(|previous| previous != cursor)
        {
            page.warnings
                .push("Multiple bottom cursors; only the last continuation is exposed".into());
        }
        page.next_cursor = Some(cursor.into());
    } else if kind != "Top" {
        page.warnings
            .push("Additional cursor branch not traversed".into());
    }
    Ok(())
}
fn content(value: &Value, page: &mut UsersPage) -> Result<()> {
    if value.get("cursorType").is_some() {
        return cursor(value, page);
    }
    if let Some(items) = value.get("items") {
        for entry in items
            .as_array()
            .ok_or_else(|| protocol().at(Diagnostic::TimelineEntry))?
        {
            item(&entry["item"]["itemContent"], page)?;
        }
        return Ok(());
    }
    if let Some(value) = value.get("itemContent") {
        return item(value, page);
    }
    Err(protocol().at(Diagnostic::TimelineEntry))
}
fn item(value: &Value, page: &mut UsersPage) -> Result<()> {
    if value
        .get("promotedMetadata")
        .is_some_and(|metadata| !metadata.is_null())
    {
        // No promotion schema is verified for relationship items. Do not turn
        // malformed/promoted data into a cacheable empty relationship result.
        return Err(protocol().at(Diagnostic::TimelineItem));
    }
    match value["itemType"].as_str() {
        Some("TimelineUser") => {
            let result = value
                .pointer("/user_results/result")
                .ok_or_else(|| protocol().at(Diagnostic::TimelineItem))?;
            // The reviewed user formatter requires core. Do not use the legacy
            // fallback accepted for historical post-author projections.
            if result["__typename"] != "User"
                || result
                    .pointer("/core/screen_name")
                    .and_then(Value::as_str)
                    .is_none()
            {
                return Err(protocol().at(Diagnostic::Identity));
            }
            page.users.push(parse_identity(result)?);
            Ok(())
        }
        Some("TimelineTimelineCursor") => cursor(value, page),
        _ => Err(protocol().at(Diagnostic::TimelineItem)),
    }
}
