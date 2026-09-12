use super::*;
use crate::error::{Error, Kind};

fn page(cursor: Option<&str>) -> UsersPage {
    UsersPage {
        users: vec![Identity {
            id: "456".into(),
            handle: "fixture".into(),
        }],
        next_cursor: cursor.map(str::to_owned),
        warnings: vec![],
    }
}
#[test]
fn users_deduplicate_ids_and_stop_repeated_cursors() {
    let mut calls = 0;
    let out = collect_users(
        Output::new("graphql", Some("123".into())),
        5,
        None,
        |cursor| {
            if calls > 0 {
                assert_eq!(cursor, Some("next"));
            }
            calls += 1;
            Ok(page(Some("next")))
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(out.users.as_ref().unwrap().len(), 1);
    assert!(out.posts.is_empty() && !out.complete && !out.request_failed);
    assert_eq!(out.stop_reason, "repeated_cursor");
}
#[test]
fn user_empty_and_bounded_success_are_not_exhaustive() {
    let out = collect_users(Output::new("graphql", Some("123".into())), 1, None, |_| {
        Ok(UsersPage {
            users: vec![],
            next_cursor: None,
            warnings: vec![],
        })
    })
    .unwrap();
    assert_eq!(out.users, Some(vec![]));
    assert!(!out.complete && !out.request_failed);
    let out = collect_users(Output::new("graphql", Some("123".into())), 1, None, |_| {
        Ok(page(Some("next")))
    })
    .unwrap();
    assert_eq!(out.stop_reason, "page_limit");
    assert_eq!(out.next_cursor.as_deref(), Some("next"));
}
#[test]
fn user_later_failures_preserve_data_without_retry() {
    let mut calls = 0;
    let out = collect_users(Output::new("graphql", Some("123".into())), 5, None, |_| {
        calls += 1;
        if calls == 1 {
            Ok(page(Some("next")))
        } else {
            Err(Error::new(Kind::RateLimit, "Synthetic cooldown"))
        }
    })
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(out.users.unwrap().len(), 1);
    assert!(out.request_failed && !out.complete);
    assert_eq!(out.next_cursor.as_deref(), Some("next"));
    assert!(
        collect_users(Output::new("graphql", None), 1, None, |_| Err(Error::new(
            Kind::Network,
            "Synthetic failure"
        )))
        .is_err()
    );
}
