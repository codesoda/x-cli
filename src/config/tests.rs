use super::*;
fn identity(id: &str, handle: &str) -> Identity {
    Identity {
        id: id.into(),
        handle: handle.into(),
    }
}
fn add(c: &mut Config, profile: &str, alias: &str, id: &str, handle: &str) -> Connection {
    c.add(profile.into(), Some(alias.into()), identity(id, handle))
        .unwrap()
}
#[test]
fn aliases_handles_defaults_and_renames() {
    let mut c = Config::default();
    let a = add(&mut c, "Default", "work", "1", "Alice");
    let b = add(&mut c, "Profile 1", "alice", "2", "Bob");
    assert_eq!(c.resolve(Some("@ALICE"), None).unwrap().id, a.id);
    assert_eq!(c.resolve(Some("alice"), None).unwrap().id, b.id);
    assert_eq!(c.resolve(None, None).unwrap().id, a.id);
    assert!(c.resolve(Some("Bob"), None).is_err());
    c.set_default("alice").unwrap();
    assert_eq!(c.default.as_deref(), Some(b.id.as_str()));
    c.rename(&b.id, Some("personal".into())).unwrap();
    assert_eq!(c.resolve(None, None).unwrap().id, b.id);
    assert!(c.rename(&b.id, Some("WORK".into())).is_err());
    assert_eq!(c.resolve(Some("personal"), None).unwrap().id, b.id);
    assert!(c.resolve(Some("work"), Some(&b.id)).is_err());
    assert!(c.set_default("missing").is_err());
    assert_eq!(c.default.as_deref(), Some(b.id.as_str()));
}
#[test]
fn same_id_aliases_do_not_bypass_connection_ambiguity() {
    let mut c = Config::default();
    let a = add(&mut c, "Default", "first", "1", "alice");
    let b = add(&mut c, "Profile 1", "second", "1", "renamed");
    for selector in [None, Some("first"), Some("second"), Some("@alice")] {
        assert!(c.resolve(selector, None).is_err());
    }
    assert_eq!(c.resolve(Some("first"), Some(&b.id)).unwrap().id, b.id);
    c.prefer(&a.id).unwrap();
    assert_eq!(c.resolve(Some("second"), None).unwrap().id, a.id);
    c.prefer(&b.id).unwrap();
    assert!(!c.connections[0].preferred);
    assert_eq!(c.resolve(Some("@alice"), None).unwrap().id, b.id);
    assert_eq!(c.resolve(None, Some(&a.id)).unwrap().id, a.id);
}
#[test]
fn local_account_resolution_needs_no_profile_preference() {
    let mut c = Config::default();
    let a = add(&mut c, "Default", "first", "1", "alice");
    let b = add(&mut c, "Profile 1", "second", "1", "renamed");
    add(&mut c, "Profile 2", "alice", "2", "bob");
    for selector in ["FIRST", "second", "@ALICE", "@renamed"] {
        assert_eq!(c.resolve_account_id(selector, None).unwrap(), "1");
        assert!(c.resolve(Some(selector), None).is_err());
    }
    assert_eq!(c.resolve_account_id("alice", None).unwrap(), "2");
    assert_eq!(c.resolve_account_id("first", Some(&b.id)).unwrap(), "1");
    assert_eq!(c.resolve_account_id("@alice", Some(&a.id)).unwrap(), "1");
    // Explicit handle matching is connection-specific, even for the same ID.
    assert!(c.resolve_account_id("@alice", Some(&b.id)).is_err());
    for selector in ["", "missing", "@missing", "@", "bob"] {
        assert!(c.resolve_account_id(selector, None).is_err());
        assert!(c.resolve_account_id(selector, Some(&a.id)).is_err());
    }
    assert!(c.resolve_account_id("alice", Some(&a.id)).is_err());
    assert!(c.resolve_account_id("first", Some("missing")).is_err());
}
#[test]
fn local_account_resolution_rejects_reused_handles_and_invalid_config() {
    let mut c = Config::default();
    let a = add(&mut c, "Default", "first", "1", "same");
    let b = add(&mut c, "Profile 1", "second", "2", "same");
    assert!(c.resolve_account_id("@same", None).is_err());
    assert_eq!(c.resolve_account_id("@SAME", Some(&a.id)).unwrap(), "1");
    assert_eq!(c.resolve_account_id("@same", Some(&b.id)).unwrap(), "2");
    c.default = Some("missing".into());
    assert!(c.resolve_account_id("first", None).is_err());
    assert!(c.resolve_account_id("first", Some(&a.id)).is_err());
    assert!(Config::default().resolve_account_id("first", None).is_err());
}
#[test]
fn reject_invalid_profiles_aliases_and_duplicates_without_mutating() {
    let mut c = Config::default();
    for profile in [
        "../Default",
        "/Default",
        "Profile 1/sub",
        "Guest Profile",
        "Profile -1",
        "Profile 01",
        "Profile ",
    ] {
        assert!(c.add(profile.into(), None, identity("1", "a")).is_err());
    }
    for alias in ["@alice", "", "a/b", "has space"] {
        assert!(
            c.add("Default".into(), Some(alias.into()), identity("1", "a"))
                .is_err()
        );
    }
    assert!(c.connections.is_empty());
    add(&mut c, "Default", "work", "1", "a");
    assert!(
        c.add("Profile 1".into(), Some("WORK".into()), identity("2", "b"))
            .is_err()
    );
    assert!(c.add("Default".into(), None, identity("2", "b")).is_err());
    assert_eq!(c.connections.len(), 1);
}
#[test]
fn identity_verification_uses_id_not_handle() {
    assert!(verify_identity(&identity("1", "old"), &identity("1", "new")).is_ok());
    assert_eq!(
        verify_identity(&identity("1", "same"), &identity("2", "same"))
            .unwrap_err()
            .kind,
        Kind::IdentityMismatch
    );
}
#[test]
fn reused_handle_requires_connection() {
    let mut c = Config::default();
    let a = add(&mut c, "Default", "a", "1", "same");
    add(&mut c, "Profile 1", "b", "2", "same");
    assert!(c.resolve(Some("@same"), None).is_err());
    assert_eq!(c.resolve(Some("@same"), Some(&a.id)).unwrap().id, a.id);
}
#[test]
fn concurrent_updates_and_explicit_removal() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    std::thread::scope(|scope| {
        for i in 1..=4 {
            let root = &root;
            scope.spawn(move || {
                Config::update(root, |c| {
                    c.add(
                        format!("Profile {i}"),
                        Some(format!("a{i}")),
                        identity(&i.to_string(), &format!("user{i}")),
                    )
                })
                .unwrap()
            });
        }
    });
    let config = Config::load(&root).unwrap();
    assert_eq!(config.connections.len(), 4);
    let default = config.default.unwrap();
    Config::update(&root, |c| c.remove(&default)).unwrap();
    let config = Config::load(&root).unwrap();
    assert_eq!(config.connections.len(), 3);
    assert!(config.default.is_none());
    assert!(config.resolve(None, Some(&default)).is_err());
}
#[test]
fn roundtrip_and_validate_loaded_config() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let mut c = Config::load(&root).unwrap();
    let a = add(&mut c, "Default", "work", "1", "@alice");
    c.save(&root).unwrap();
    assert_eq!(
        Config::load(&root).unwrap().resolve(None, None).unwrap().id,
        a.id
    );
    c.default = Some("missing".into());
    state::write_json(&root.join("config.json"), &c).unwrap();
    assert!(Config::load(&root).is_err());
    c.default = None;
    c.connections[0].profile = "../escape".into();
    state::write_json(&root.join("config.json"), &c).unwrap();
    assert!(Config::load(&root).is_err());
    assert!(c.save(&root).is_err());
}
