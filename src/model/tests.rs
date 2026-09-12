use super::*;

#[test]
fn legacy_post_output_omits_users_and_still_deserializes() {
    let output = Output::new("fxtwitter", None);
    let value = serde_json::to_value(&output).unwrap();
    assert!(value.get("users").is_none());
    assert_eq!(value["posts"], serde_json::json!([]));
    let restored: Output = serde_json::from_value(value).unwrap();
    assert!(restored.users.is_none());
}

#[test]
fn users_are_explicit_even_when_empty_and_render_as_profiles() {
    let mut output = Output::new("graphql", Some("123".into()));
    output.users = Some(vec![]);
    assert_eq!(
        serde_json::to_value(&output).unwrap()["users"],
        serde_json::json!([])
    );
    output.users.as_mut().unwrap().push(Identity {
        id: "456".into(),
        handle: "fixture".into(),
    });
    let value = serde_json::to_value(&output).unwrap();
    let rendered = crate::app::human(&value);
    assert!(rendered.contains("@fixture · 456"));
    assert!(rendered.contains("https://x.com/fixture"));
    assert!(!rendered.contains("/status/"));
    assert!(output.posts.is_empty());
}

#[test]
fn user_collections_roundtrip_through_private_scoped_cache() {
    let temp = tempfile::tempdir().unwrap();
    let cache = crate::cache::Cache::new(temp.path().canonicalize().unwrap().join("cache"));
    let mut output = Output::new("graphql", Some("123".into()));
    output.users = Some(vec![Identity {
        id: "456".into(),
        handle: "fixture".into(),
    }]);
    cache.put("graphql", Some("123"), "users", &output).unwrap();
    let hit = cache
        .get("graphql", Some("123"), "users", 300)
        .unwrap()
        .unwrap();
    assert_eq!(hit.users, output.users);
    assert!(hit.posts.is_empty());
    assert_eq!(hit.provenance.cache, "hit");
    assert!(
        cache
            .get("graphql", Some("999"), "users", 300)
            .unwrap()
            .is_none()
    );
    assert!(cache.get("graphql", None, "users", 300).unwrap().is_none());
    cache.purge().unwrap();
    assert!(
        cache
            .get("graphql", Some("123"), "users", 300)
            .unwrap()
            .is_none()
    );
}
