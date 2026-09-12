use super::{Items, collect_items};
use crate::{
    error::Result,
    model::{ListInfo, Output},
};

pub struct ListsPage {
    pub lists: Vec<ListInfo>,
    pub next_cursor: Option<String>,
    pub warnings: Vec<String>,
}

/// Collect a visible snapshot, preserving overlapping placement provenance.
/// Conflicting observations are not authoritative reconciliation evidence.
pub fn collect_lists(
    mut out: Output,
    max_pages: u32,
    start: Option<String>,
    mut fetch: impl FnMut(Option<&str>) -> Result<ListsPage>,
) -> Result<Output> {
    out.posts.clear();
    out.users = None;
    out.lists = Some(vec![]);
    collect_items(
        out,
        max_pages,
        start,
        |cursor| {
            let page = fetch(cursor)?;
            Ok(Items {
                entries: page.lists,
                next_cursor: page.next_cursor,
                warnings: page.warnings,
            })
        },
        |list: &ListInfo| list.id.as_str(),
        |out, list| {
            out.lists
                .as_mut()
                .expect("list collection initialized")
                .push(list);
        },
        |out, index, later| {
            let first = &mut out.lists.as_mut().expect("list collection initialized")[index];
            if merge(first, later) {
                out.warnings.push("Duplicate list metadata conflicted; first known values retained in this visible snapshot".into());
            }
        },
    )
}

fn merge(first: &mut ListInfo, later: ListInfo) -> bool {
    let mut conflict = first.name != later.name || first.url != later.url;
    conflict |= fill(&mut first.description, later.description);
    conflict |= fill(&mut first.visibility, later.visibility);
    conflict |= fill(&mut first.owner, later.owner);
    conflict |= fill(&mut first.subscribed, later.subscribed);
    conflict |= fill(&mut first.pinned, later.pinned);
    conflict |= fill(&mut first.is_member, later.is_member);
    for section in later.management_sections {
        if !first.management_sections.contains(&section) {
            first.management_sections.push(section);
        }
    }
    conflict
}

fn fill<T: PartialEq>(first: &mut Option<T>, later: Option<T>) -> bool {
    match (first.as_ref(), later) {
        (Some(first), Some(later)) => first != &later,
        (None, Some(later)) => {
            *first = Some(later);
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
