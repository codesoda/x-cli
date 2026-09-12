use super::*;

#[test]
#[ignore = "local list metadata read; requires consent, account/profile and list ID"]
fn authenticated_list_metadata_smoke() {
    require_opt_in(true);
    let list_id = required("XCLI_LIVE_LIST_ID");
    xcli::input::id(&list_id).expect("XCLI_LIVE_LIST_ID must be a positive decimal list ID");
    let (root, account, connection) = authenticated_connection();
    let output = run(
        &root,
        &[
            "lists",
            "show",
            &list_id,
            "--account",
            &account,
            "--connection",
            &connection.id,
        ],
        "authenticated list metadata",
    );
    assert!(output.provenance.backend == "graphql");
    assert!(
        output.provenance.account_id.as_ref() == Some(&connection.identity.id),
        "Account provenance mismatch; values withheld"
    );
    assert!(output.complete && output.pages == 1 && output.stop_reason == "single_list");
    assert!(output.posts.is_empty() && output.users.is_none() && output.next_cursor.is_none());
    let lists = output
        .lists
        .expect("Expected list metadata; payload withheld");
    assert!(
        lists.len() == 1 && lists[0].id == list_id,
        "Expected the requested single list; IDs withheld"
    );
    assert!(lists[0].url == format!("https://x.com/i/lists/{list_id}"));
    // Optional owner/visibility/description are not permission or actor evidence.
    // No metadata values or raw stderr are printed; cooldowns remain intact.
}
