use super::super::list_inventory::parse;
use super::*;
use crate::model::ListManagementSection::{OwnedSubscribed, Pinned, Unsectioned};

mod requests;

fn raw() -> Value {
    json!({"id_str":"456","name":"Fixture","mode":"Private"})
}
fn item(list: Value) -> Value {
    json!({"itemType":"TimelineTwitterList","list":list})
}
fn flat(list: Value) -> Value {
    json!({"entryId":"list-456","content":{"itemContent":item(list)}})
}
fn module(id: &str, list: Value) -> Value {
    json!({"entryId":id,"content":{"entryType":"TimelineTimelineModule",
        "items":[{"item":{"itemContent":item(list)}}]}})
}
fn instructions(entries: Vec<Value>) -> Value {
    json!([{"type":"TimelineAddEntries","entries":entries}])
}
fn body(instructions: Value) -> Value {
    json!({"data":{"viewer":{"list_management_timeline":{"timeline":{"instructions":instructions}}}}})
}

#[test]
fn list_inventory_module_placements_are_overlapping_not_ownership_inferences() {
    for (prefix, expected) in [
        ("pinned-list-module", Pinned),
        ("pinnedListModule", Pinned),
        ("owned-subscribed-list-module", OwnedSubscribed),
        ("ownedSubscribedListModule", OwnedSubscribed),
    ] {
        let value = instructions(vec![module(&format!("{prefix}-0"), raw())]);
        let page = parse(&value, "123").unwrap();
        assert_eq!(page.lists[0].management_sections, vec![expected]);
        assert!(page.lists[0].pinned.is_none());
        assert!(page.lists[0].subscribed.is_none());
        assert!(page.lists[0].owner.is_none());
    }
    let page = parse(
        &instructions(vec![
            module("pinned-list-module", raw()),
            module("ownedSubscribedListModule", raw()),
        ]),
        "123",
    )
    .unwrap();
    assert_eq!(page.lists.len(), 2); // Collector merges, parser preserves observations.
    assert_eq!(page.lists[0].management_sections, vec![Pinned]);
    assert_eq!(page.lists[1].management_sections, vec![OwnedSubscribed]);
}

#[test]
fn list_inventory_flat_rows_require_positive_inventory_evidence() {
    for field in ["pinning", "following"] {
        let mut list = raw();
        list[field] = json!(true);
        let page = parse(&instructions(vec![flat(list)]), "123").unwrap();
        assert_eq!(page.lists[0].management_sections, vec![Unsectioned]);
    }
    let mut list = raw();
    list["user_results"] = json!({"result":{"__typename":"User","rest_id":"123",
        "core":{"screen_name":"fixture"}}});
    let page = parse(&instructions(vec![flat(list.clone())]), "123").unwrap();
    assert_eq!(page.lists[0].management_sections, vec![Unsectioned]);
    assert!(parse(&instructions(vec![flat(list)]), "999").is_err());
    for patch in [
        json!({}),
        json!({"pinning":false,"following":false}),
        json!({"is_member":true}),
        json!({"displayType":"Pinned"}),
        json!({"following":null,"pinning":null}),
    ] {
        let mut list = raw();
        list.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(parse(&instructions(vec![flat(list)]), "123").is_err());
    }
}

#[test]
fn list_inventory_direct_shape_and_strict_modules_reject_recommendations() {
    let valid = module("pinned-list-module", raw());
    for entry in [
        module("recommended-lists-module", raw()),
        module(
            "ListsDiscovery",
            json!({"id_str":"456","name":"Fixture","pinning":true}),
        ),
        json!({"content":{"itemContent":{"itemType":"TimelineTwitterList",
            "list_results":{"result":raw()}}}}),
        json!({"content":{"itemContent":{"list":raw()}}}),
        module(
            "pinned-list-module",
            json!({"rest_id":"456","name":"Fixture"}),
        ),
        module("pinned-list-module", Value::Null),
        json!({"entryId":"pinned-list-module","content":{"items":{}}}),
        json!({"entryId":"pinned-list-module","content":{"items":[{"itemContent":item(raw())}]}}),
    ] {
        assert!(parse(&instructions(vec![entry]), "123").is_err());
    }
    for path in [
        "/promotedMetadata",
        "/content/promotedMetadata",
        "/content/items/0/promotedMetadata",
        "/content/items/0/item/promotedMetadata",
        "/content/items/0/item/itemContent/promotedMetadata",
        "/content/items/0/item/itemContent/list/promotedMetadata",
    ] {
        // Add the unsupported promotion field to each reviewed container.
        let (parent, field) = path.rsplit_once('/').unwrap();
        let mut value = valid.clone();
        value.pointer_mut(parent).unwrap()[field] = json!({});
        assert!(parse(&instructions(vec![value.clone()]), "123").is_err());
        value.pointer_mut(path).unwrap().clone_from(&Value::Null);
        assert!(parse(&instructions(vec![value]), "123").is_ok());
    }
}

#[test]
fn list_inventory_reviewed_instruction_forms_and_bottom_cursors() {
    let value = json!([
        {"type":"TimelineAddEntries","entries":[]},
        {"type":"TimelineReplaceEntry","entry":module("pinnedListModule", raw())},
        {"type":"TimelinePinEntry","entry":module("ownedSubscribedListModule", raw())},
        {"type":"TimelineAddToModule","moduleEntryId":"pinned-list-module",
            "moduleItems":[{"item":{"itemContent":item(raw())}}]},
        {"type":"TimelineReplaceEntry","entry":{"content":{"cursorType":"Bottom","value":"opaque + / ="}}},
        {"type":"TimelineTerminateTimeline"}, {"type":"TimelineClearCache"}
    ]);
    let page = parse(&value, "123").unwrap();
    assert_eq!(page.lists.len(), 3);
    assert_eq!(page.lists[2].management_sections, vec![Pinned]);
    assert_eq!(page.next_cursor.as_deref(), Some("opaque + / ="));
    assert!(
        parse(&instructions(vec![]), "123")
            .unwrap()
            .lists
            .is_empty()
    );
    for value in [
        json!([]),
        Value::Null,
        json!({}),
        json!([{"type":"TimelineAddEntries"}]),
        json!([{"type":"TimelineShowAlert"}]),
        json!([{"type":"TimelineRemoveEntries"}]),
        json!([{"type":"unknown"}]),
        json!([{"type":"TimelineAddToModule","moduleEntryId":"discovery","moduleItems":[{"item":{"itemContent":item(raw())}}]}]),
        instructions(vec![json!({"content":{"cursorType":"Bottom","value":7}})]),
    ] {
        assert!(parse(&value, "123").is_err());
    }
}

#[test]
fn list_inventory_and_metadata_optional_booleans_preserve_unknown_and_false() {
    for (field, normalized) in [
        ("following", "subscribed"),
        ("pinning", "pinned"),
        ("is_member", "is_member"),
    ] {
        for boolean in [Value::Null, json!(false), json!(true)] {
            let mut list = raw();
            list[field] = boolean.clone();
            let parsed = super::super::list_metadata::parse(&list).unwrap();
            assert!(parsed.management_sections.is_empty());
            let output = serde_json::to_value(parsed).unwrap();
            if boolean.is_null() {
                assert!(output.get(normalized).is_none());
            } else {
                assert_eq!(output[normalized], boolean);
            }
            let page = parse(&instructions(vec![module("pinnedListModule", list)]), "123").unwrap();
            assert_eq!(
                serde_json::to_value(&page.lists[0]).unwrap()[normalized],
                output[normalized]
            );
        }
        for invalid in [json!("false"), json!(0), json!([]), json!({})] {
            let mut list = raw();
            list[field] = invalid;
            assert!(super::super::list_metadata::parse(&list).is_err());
            assert!(parse(&instructions(vec![module("pinnedListModule", list)]), "123").is_err());
        }
    }
}
