use super::*;

#[test]
fn newly_created_directory_chain_is_synced_before_descending() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let first = root.join("first");
    let second = first.join("second");
    let path = second.join("record.json");
    let mut calls = 0;
    let result = parent_with_sync(&path, true, |parent, child| {
        calls += 1;
        assert!(parent.metadata().unwrap().is_dir());
        assert_eq!(child.metadata().unwrap().mode() & 0o777, 0o700);
        assert!(!path.exists());
        match calls {
            1 => assert!(first.is_dir() && !second.exists()),
            2 => assert!(second.is_dir()),
            _ => panic!("unexpected directory creation"),
        }
        sync_created_directory(parent, child)
    })
    .unwrap();
    assert!(result.is_some());
    assert_eq!(calls, 2);
    // Existing directory walks need no creation-sync callbacks.
    parent_with_sync(&path, true, |_, _| panic!("existing directory resynced")).unwrap();
    super::super::write_json(&path, &serde_json::json!({"synthetic":true})).unwrap();
    let restored: serde_json::Value = super::super::read_json(&path).unwrap().unwrap();
    assert_eq!(restored, serde_json::json!({"synthetic":true}));
}

#[test]
fn directory_sync_failure_stops_before_descendant_or_record_creation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let first = root.join("first");
    let second = first.join("second");
    let path = second.join("record.json");
    let mut calls = 0;
    let error = parent_with_sync(&path, true, |_, child| {
        calls += 1;
        child.sync_all().map_err(|_| storage())?;
        // Inject a containing-directory sync failure, without global hooks.
        Err(storage())
    })
    .unwrap_err();
    assert_eq!(error.kind, crate::error::Kind::Storage);
    assert_eq!(calls, 1);
    assert!(first.is_dir());
    assert!(!second.exists() && !path.exists());
}

#[test]
fn read_only_missing_directory_walk_creates_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let missing = root.join("missing");
    assert!(
        parent_with_sync(&missing.join("record.json"), false, |_, _| {
            panic!("read-only walk attempted directory creation")
        })
        .unwrap()
        .is_none()
    );
    assert!(!missing.exists());
}
