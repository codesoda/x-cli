use super::*;

#[test]
#[ignore = "local private list inventory read; requires consent and explicit account/profile"]
fn authenticated_list_inventory_smoke() {
    require_opt_in(true);
    let (root, account, connection) = authenticated_connection();
    let output = run(
        &root,
        &[
            "lists",
            "list",
            "--account",
            &account,
            "--connection",
            &connection.id,
            "--max-pages",
            "1",
            "--page-size",
            "5",
        ],
        "authenticated list inventory",
    );
    assert!(output.provenance.backend == "graphql");
    assert!(
        output.provenance.account_id.as_ref() == Some(&connection.identity.id),
        "Account provenance mismatch; values withheld"
    );
    assert!(output.pages == 1 && !output.complete);
    assert!(output.posts.is_empty() && output.users.is_none());
    let lists = output
        .lists
        .expect("Expected list inventory; payload withheld");
    assert!(
        lists
            .iter()
            .all(|list| !list.management_sections.is_empty()),
        "Missing management placement provenance; payload withheld"
    );
    // Empty inventories are valid incomplete views. No private list values are printed.
    // This is not exhaustive ownership, authorization or reconciliation evidence.
}
