//! Strict projection of the non-Relay viewer management timeline, not recommendations.
use super::list_metadata;
use crate::{
    error::{Diagnostic, Result, protocol},
    model::ListManagementSection,
    pagination::ListsPage,
};
use serde_json::Value;

pub(super) fn parse(value: &Value, actor: &str) -> Result<ListsPage> {
    let instructions = value
        .as_array()
        .ok_or_else(|| protocol().at(Diagnostic::TimelineInstructions))?;
    let mut page = ListsPage {
        lists: vec![],
        next_cursor: None,
        warnings: vec![],
    };
    let mut recognized = false;
    for instruction in instructions {
        reject_promotion(instruction)?;
        match instruction["type"].as_str() {
            Some("TimelineAddEntries") => {
                for value in instruction["entries"]
                    .as_array()
                    .ok_or_else(|| protocol().at(Diagnostic::TimelineInstructions))?
                {
                    entry(value, actor, &mut page)?;
                }
            }
            Some("TimelineReplaceEntry" | "TimelinePinEntry") => {
                entry(&instruction["entry"], actor, &mut page)?;
            }
            Some("TimelineAddToModule") => {
                let section = section(&instruction["moduleEntryId"])?;
                module_items(&instruction["moduleItems"], section, actor, &mut page)?;
            }
            Some("TimelineTerminateTimeline" | "TimelineClearCache") => {}
            _ => return Err(protocol().at(Diagnostic::TimelineInstruction)),
        }
        recognized = true;
    }
    if !recognized {
        return Err(protocol().at(Diagnostic::TimelineInstructions));
    }
    Ok(page)
}

fn section(id: &Value) -> Result<ListManagementSection> {
    let id = id
        .as_str()
        .ok_or_else(|| protocol().at(Diagnostic::TimelineEntry))?;
    if id.starts_with("pinned-list-module") || id.starts_with("pinnedListModule") {
        Ok(ListManagementSection::Pinned)
    } else if id.starts_with("owned-subscribed-list-module")
        || id.starts_with("ownedSubscribedListModule")
    {
        Ok(ListManagementSection::OwnedSubscribed)
    } else {
        // Unknown modules could contain discovery/recommendation lists. Never guess.
        Err(protocol().at(Diagnostic::TimelineEntry))
    }
}

fn entry(value: &Value, actor: &str, page: &mut ListsPage) -> Result<()> {
    reject_promotion(value)?;
    let content = &value["content"];
    reject_promotion(content)?;
    if content.get("cursorType").is_some() {
        return cursor(content, page);
    }
    if let Some(items) = content.get("items") {
        return module_items(items, section(&value["entryId"])?, actor, page);
    }
    item(&content["itemContent"], None, actor, page)
}

fn module_items(
    items: &Value,
    section: ListManagementSection,
    actor: &str,
    page: &mut ListsPage,
) -> Result<()> {
    for value in items
        .as_array()
        .ok_or_else(|| protocol().at(Diagnostic::TimelineEntry))?
    {
        reject_promotion(value)?;
        reject_promotion(&value["item"])?;
        item(&value["item"]["itemContent"], Some(&section), actor, page)?;
    }
    Ok(())
}

fn item(
    value: &Value,
    section: Option<&ListManagementSection>,
    actor: &str,
    page: &mut ListsPage,
) -> Result<()> {
    reject_promotion(value)?;
    match value["itemType"].as_str() {
        Some("TimelineTwitterList") => {
            // The reviewed item normalizer reads direct list, not list_results.result.
            let raw = &value["list"];
            reject_promotion(raw)?;
            let mut list = list_metadata::parse(raw)?;
            let placement = match section {
                Some(section) => section.clone(),
                None if list.pinned == Some(true)
                    || list.subscribed == Some(true)
                    || list.owner.as_ref().is_some_and(|owner| owner.id == actor) =>
                {
                    ListManagementSection::Unsectioned
                }
                None => return Err(protocol().at(Diagnostic::TimelineItem)),
            };
            list.management_sections.push(placement);
            page.lists.push(list);
            Ok(())
        }
        Some("TimelineTimelineCursor") => cursor(value, page),
        _ => Err(protocol().at(Diagnostic::TimelineItem)),
    }
}

fn reject_promotion(value: &Value) -> Result<()> {
    if value
        .get("promotedMetadata")
        .is_some_and(|metadata| !metadata.is_null())
    {
        return Err(protocol().at(Diagnostic::TimelineItem));
    }
    Ok(())
}

fn cursor(value: &Value, page: &mut ListsPage) -> Result<()> {
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
