use crate::{
    error::Result,
    model::{Output, Post},
};
use std::collections::HashSet;

pub struct Page {
    pub posts: Vec<Post>,
    pub next_cursor: Option<String>,
    pub warnings: Vec<String>,
}
/// Collect a bounded view. Cursor exhaustion never proves all X content was exposed.
pub fn collect(
    mut out: Output,
    max_pages: u32,
    start: Option<String>,
    mut fetch: impl FnMut(Option<&str>) -> Result<Page>,
) -> Result<Output> {
    let mut cursor = start;
    let mut cursors = HashSet::new();
    let mut posts = HashSet::new();
    if let Some(c) = &cursor {
        cursors.insert(c.clone());
    }
    for _ in 0..max_pages {
        let page = match fetch(cursor.as_deref()) {
            Ok(p) => p,
            Err(e) if out.pages > 0 => {
                out.request_failed = true;
                out.stop_reason = "request_failed".into();
                out.next_cursor = cursor;
                out.warnings
                    .push(format!("{} (exit code {})", e.message, e.exit_code()));
                if let Some(s) = e.retry_after_seconds {
                    out.warnings.push(format!("Retry after {s} seconds"));
                }
                return Ok(out);
            }
            Err(e) => return Err(e),
        };
        out.pages += 1;
        out.warnings.extend(page.warnings);
        for p in page.posts {
            if posts.insert(p.id.clone()) {
                out.posts.push(p);
            }
        }
        cursor = page.next_cursor.filter(|s| !s.is_empty());
        if cursor.is_none() {
            out.stop_reason = "cursor_exhausted".into();
            break;
        }
        if !cursors.insert(cursor.clone().expect("present cursor")) {
            out.stop_reason = "repeated_cursor".into();
            out.warnings
                .push("Upstream repeated a pagination cursor".into());
            break;
        }
        out.stop_reason = "page_limit".into();
    }
    out.next_cursor = cursor;
    out.warnings.push(
        "This is an upstream-visible collection, not a complete archive or reply tree".into(),
    );
    Ok(out)
}
/// Fetch parents one by one using the same backend and identity. Unknown parent metadata stops safely.
pub fn parents(
    mut out: Output,
    max_parents: u32,
    mut read: impl FnMut(&str) -> Result<Output>,
) -> Result<Output> {
    out.complete = false;
    let mut seen: HashSet<String> = out.posts.iter().map(|p| p.id.clone()).collect();
    for depth in 0..=max_parents {
        let Some(p) = out.posts.last() else {
            return Err(crate::error::protocol());
        };
        if !p.parent_known {
            out.stop_reason = "parent_unknown".into();
            out.warnings.push("Provider omitted parent metadata".into());
            break;
        }
        let Some(id) = p.parent_id.clone() else {
            out.complete = true;
            out.stop_reason = "root_reached".into();
            break;
        };
        if depth == max_parents {
            out.stop_reason = "parent_limit".into();
            break;
        }
        if !seen.insert(id.clone()) {
            out.stop_reason = "parent_cycle".into();
            break;
        }
        match read(&id) {
            Ok(parent) => {
                let Some(post) = parent.posts.into_iter().find(|p| p.id == id) else {
                    return Err(crate::error::protocol());
                };
                out.pages += parent.pages;
                out.warnings.extend(parent.warnings);
                out.posts.push(post);
            }
            Err(e) => {
                out.request_failed = true;
                out.stop_reason = "parent_unavailable".into();
                out.warnings.push(format!(
                    "Parent retrieval stopped: {} (exit code {})",
                    e.message,
                    e.exit_code()
                ));
                break;
            }
        }
    }
    out.parent_chain_complete = Some(out.complete);
    out.posts.reverse();
    if !out.complete {
        out.warnings.push("Parent chain is partial".into());
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::{Error, Kind},
        model::Identity,
    };
    fn post(id: &str, parent: Option<&str>) -> Post {
        Post {
            id: id.into(),
            author: Identity {
                id: "1".into(),
                handle: "test".into(),
            },
            text: "".into(),
            created_at: None,
            parent_id: parent.map(str::to_owned),
            parent_known: true,
            url: "".into(),
        }
    }
    #[test]
    fn repeat_dedup() {
        let mut n = 0;
        let out = collect(Output::new("graphql", Some("1".into())), 10, None, |_| {
            n += 1;
            Ok(Page {
                posts: vec![post("20", None)],
                next_cursor: Some("same".into()),
                warnings: vec![],
            })
        })
        .unwrap();
        assert_eq!(n, 2);
        assert_eq!(out.posts.len(), 1);
        assert_eq!(out.stop_reason, "repeated_cursor");
        assert!(!out.complete);
    }
    #[test]
    fn partial_failure() {
        let mut n = 0;
        let out = collect(Output::new("graphql", None), 3, None, |_| {
            n += 1;
            if n == 2 {
                return Err(Error::new(Kind::Authentication, "Expired"));
            }
            Ok(Page {
                posts: vec![],
                next_cursor: Some("next".into()),
                warnings: vec![],
            })
        })
        .unwrap();
        assert_eq!(out.stop_reason, "request_failed");
        assert_eq!(out.next_cursor.as_deref(), Some("next"));
        assert!(!out.complete);
        assert!(
            collect(Output::new("graphql", None), 1, None, |_| Err(Error::new(
                Kind::Network,
                "Failed"
            )))
            .is_err()
        );
    }
    #[test]
    fn parent_limit() {
        let mut o = Output::new("fxtwitter", None);
        o.posts.push(post("22", Some("21")));
        let o = parents(o, 1, |_| {
            let mut o = Output::new("fxtwitter", None);
            o.posts.push(post("21", Some("20")));
            Ok(o)
        })
        .unwrap();
        assert!(!o.complete);
        assert_eq!(o.stop_reason, "parent_limit");
    }
    #[test]
    fn root() {
        let mut o = Output::new("fxtwitter", None);
        o.posts.push(post("20", None));
        assert!(parents(o, 1, |_| unreachable!()).unwrap().complete);
    }
}
