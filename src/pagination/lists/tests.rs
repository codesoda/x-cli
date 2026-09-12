use super::*;
use crate::{
    error::{Error, Kind},
    model::{
        Identity,
        ListManagementSection::{OwnedSubscribed, Pinned, Unsectioned},
        ListVisibility,
    },
};

fn list() -> ListInfo {
    ListInfo {
        id: "456".into(),
        name: "Fixture".into(),
        description: None,
        visibility: None,
        owner: None,
        subscribed: None,
        pinned: None,
        is_member: None,
        management_sections: vec![Pinned],
        url: "https://x.com/i/lists/456".into(),
    }
}
fn page(lists: Vec<ListInfo>, cursor: Option<&str>) -> ListsPage {
    ListsPage {
        lists,
        next_cursor: cursor.map(str::to_owned),
        warnings: vec![],
    }
}

#[test]
fn duplicate_lists_union_sections_fill_missing_and_retain_explicit_false() {
    let mut first = list();
    first.subscribed = Some(false);
    let mut second = list();
    second.description = Some("Fixture description".into());
    second.visibility = Some(ListVisibility::Private);
    second.owner = Some(Identity {
        id: "123".into(),
        handle: "fixture".into(),
    });
    second.pinned = Some(false);
    second.is_member = Some(false);
    second.management_sections = vec![OwnedSubscribed, Pinned];
    let mut third = list();
    third.management_sections = vec![Unsectioned, OwnedSubscribed];
    let mut pages = [
        page(vec![first, second.clone()], Some("next")),
        page(vec![third], None),
    ]
    .into_iter();
    let out = collect_lists(Output::new("graphql", Some("123".into())), 2, None, |_| {
        Ok(pages.next().unwrap())
    })
    .unwrap();
    assert!(!out.complete);
    let lists = out.lists.unwrap();
    assert_eq!(lists.len(), 1);
    let result = &lists[0];
    assert_eq!(
        result.management_sections,
        vec![Pinned, OwnedSubscribed, Unsectioned]
    );
    assert_eq!(result.subscribed, Some(false));
    assert_eq!(result.pinned, Some(false));
    assert_eq!(result.is_member, Some(false));
    assert_eq!(result.description, second.description);
    assert_eq!(result.visibility, second.visibility);
    assert_eq!(result.owner, second.owner);
    assert!(
        !out.warnings
            .iter()
            .any(|warning| warning.contains("conflicted"))
    );
}

#[test]
fn duplicate_list_conflicts_keep_first_known_fields_with_static_warning() {
    let mut first = list();
    first.description = Some("First".into());
    first.visibility = Some(ListVisibility::Private);
    first.owner = Some(Identity {
        id: "123".into(),
        handle: "first".into(),
    });
    first.subscribed = Some(false);
    first.pinned = Some(true);
    first.is_member = Some(false);
    let mut later = first.clone();
    later.name = "SYNTHETIC_PRIVATE_NAME".into();
    later.description = Some("SYNTHETIC_PRIVATE_DESCRIPTION".into());
    later.visibility = Some(ListVisibility::Public);
    later.owner = Some(Identity {
        id: "789".into(),
        handle: "other".into(),
    });
    later.subscribed = Some(true);
    later.pinned = Some(false);
    later.is_member = Some(true);
    later.management_sections = vec![OwnedSubscribed];
    let out = collect_lists(Output::new("graphql", None), 1, None, |_| {
        Ok(page(vec![first.clone(), later.clone()], None))
    })
    .unwrap();
    let mut expected = first;
    expected.management_sections.push(OwnedSubscribed);
    assert_eq!(out.lists.unwrap(), vec![expected]);
    assert_eq!(
        out.warnings
            .iter()
            .filter(|warning| warning.contains("conflicted"))
            .count(),
        1
    );
    assert!(!out.warnings.join(" ").contains("SYNTHETIC_PRIVATE"));
}

#[test]
fn each_optional_boolean_conflict_warns_independently_without_overwriting_false() {
    for field in ["subscribed", "pinned", "is_member"] {
        let mut first = serde_json::to_value(list()).unwrap();
        first[field] = serde_json::json!(false);
        let mut later = first.clone();
        later[field] = serde_json::json!(true);
        let mut first: ListInfo = serde_json::from_value(first).unwrap();
        let expected = first.clone();
        assert!(merge(&mut first, serde_json::from_value(later).unwrap()));
        assert_eq!(first, expected);
    }
}

#[test]
fn list_collection_bounds_repeat_empty_and_failure_semantics() {
    let mut calls = 0;
    let out = collect_lists(
        Output::new("graphql", None),
        3,
        Some("same".into()),
        |cursor| {
            calls += 1;
            assert_eq!(cursor, Some("same"));
            Ok(page(vec![list()], Some("same")))
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(out.stop_reason, "repeated_cursor");
    assert!(!out.complete);
    let out = collect_lists(Output::new("graphql", None), 1, None, |_| {
        Ok(page(vec![], Some("next")))
    })
    .unwrap();
    assert_eq!(out.stop_reason, "page_limit");
    assert_eq!(out.lists, Some(vec![]));
    assert_eq!(out.next_cursor.as_deref(), Some("next"));
    let out = collect_lists(Output::new("graphql", None), 1, None, |_| {
        Ok(page(vec![], None))
    })
    .unwrap();
    assert_eq!(out.stop_reason, "cursor_exhausted");
    assert!(!out.complete && !out.request_failed);
    assert_eq!(out.lists, Some(vec![]));
    assert!(
        collect_lists(Output::new("graphql", None), 2, None, |_| Err(Error::new(
            Kind::Network,
            "Synthetic failure"
        )))
        .is_err()
    );
    let mut calls = 0;
    let out = collect_lists(Output::new("graphql", None), 3, None, |_| {
        calls += 1;
        if calls == 1 {
            Ok(page(vec![list()], Some("next")))
        } else {
            Err(Error::new(Kind::ProtocolChanged, "Synthetic failure"))
        }
    })
    .unwrap();
    assert_eq!(calls, 2);
    assert!(out.request_failed && !out.complete);
    assert_eq!(out.lists.unwrap().len(), 1);
    assert_eq!(out.next_cursor.as_deref(), Some("next"));
}

#[test]
fn collectors_clear_unrelated_optional_collections() {
    let mut seed = Output::new("graphql", None);
    seed.users = Some(vec![Identity {
        id: "123".into(),
        handle: "fixture".into(),
    }]);
    seed.lists = Some(vec![list()]);
    let lists = collect_lists(seed.clone(), 1, None, |_| Ok(page(vec![], None))).unwrap();
    assert!(lists.posts.is_empty() && lists.users.is_none());
    assert_eq!(lists.lists, Some(vec![]));
    let posts = crate::pagination::collect(seed.clone(), 1, None, |_| {
        Ok(crate::pagination::Page {
            posts: vec![],
            next_cursor: None,
            warnings: vec![],
        })
    })
    .unwrap();
    assert!(posts.users.is_none() && posts.lists.is_none());
    let users = crate::pagination::collect_users(seed, 1, None, |_| {
        Ok(crate::pagination::UsersPage {
            users: vec![],
            next_cursor: None,
            warnings: vec![],
        })
    })
    .unwrap();
    assert!(users.posts.is_empty() && users.lists.is_none());
    assert_eq!(users.users, Some(vec![]));
}
