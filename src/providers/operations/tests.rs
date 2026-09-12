use super::Operation;
use std::collections::BTreeSet;

#[test]
fn bookmarks_feature_names_match_independent_source_fixture() {
    // Public Bookmarks metadata in shared~bundle.BookmarkFolders~bundle.Bookmarks
    // .2be1bbc341bba456a.js, SHA-256
    // c1fc402b73ee04bf5d03964285e0bbd0939cf4916f11cd3a227293fd59e84a33.
    // Keep independent of POST and other operation outputs: shared drift must fail.
    // This fixes membership, not authenticated rollout boolean values.
    let fixture: Vec<_> = include_str!("bookmark-features.txt").lines().collect();
    let expected: BTreeSet<_> = fixture.iter().copied().collect();
    assert_eq!(expected.len(), fixture.len());
    let features = Operation::Bookmarks.features();
    let actual: BTreeSet<_> = features
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(actual, expected);
    assert!(
        features
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.is_boolean())
    );
}
