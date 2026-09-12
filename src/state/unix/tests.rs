use super::*;
use directory::parent_with_sync;

fn id(file: &File) -> (u64, u64) {
    let meta = file.metadata().unwrap();
    (meta.dev(), meta.ino())
}

#[test]
fn failed_creation_and_repeated_retry_stop_write_and_lock_before_descendants() {
    for lock in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let first = root.join("first");
        let second = first.join("second");
        let path = second.join(if lock { "record.lock" } else { "record.json" });
        for attempt in 0..3 {
            let mut failed = false;
            let resolve = || {
                parent_with_sync(&path, true, |parent, child, created| {
                    assert!(!second.exists() && !path.exists());
                    if first.exists() && id(child) == id(&File::open(&first).unwrap()) {
                        assert_eq!(created, attempt == 0);
                        assert_eq!(id(parent), id(&File::open(&root).unwrap()));
                        failed = true;
                        // Model success syncing the child, then failure syncing
                        // its containing directory; no real traversal syncs.
                        return Err(storage());
                    }
                    assert!(!created);
                    Ok(())
                })
            };
            let error = if lock {
                with_lock_parent(|| panic!("lock action ran after failed walk"), resolve)
            } else {
                write_with_parent(b"{}", resolve)
            }
            .unwrap_err();
            assert_eq!(error.kind, crate::error::Kind::Storage);
            assert!(failed && first.is_dir());
            assert!(!second.exists() && !path.exists());
            assert_eq!(std::fs::read_dir(&first).unwrap().count(), 0);
        }
        // A successful retry must reprocess first before second is created.
        let mut recovered = false;
        parent_with_sync(&path, true, |_, child, created| {
            if id(child) == id(&File::open(&first).unwrap()) {
                assert!(!created && !second.exists());
                recovered = true;
            } else if created {
                assert!(recovered);
                assert_eq!(id(child), id(&File::open(&second).unwrap()));
            }
            Ok(())
        })
        .unwrap();
        assert!(recovered && second.is_dir() && !path.exists());
    }
}

#[test]
fn existing_walk_failure_preserves_published_record_and_blocks_lock_action() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let root_id = id(&File::open(&root).unwrap());
    let path = root.join("record.json");
    std::fs::write(&path, b"old").unwrap();
    let fail = |_: &File, child: &File, created: bool| {
        assert!(!created);
        if id(child) == root_id {
            Err(storage())
        } else {
            Ok(())
        }
    };
    assert!(write_with_parent(b"new", || parent_with_sync(&path, true, fail)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
    let lock = root.join("record.lock");
    assert!(
        with_lock_parent(
            || -> Result<()> { panic!("lock action ran") },
            || parent_with_sync(&lock, true, fail)
        )
        .is_err()
    );
    assert!(!lock.exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
}

#[test]
fn real_temporary_state_write_read_and_lock_succeed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = root.join("first/second/record.json");
    super::super::write_json(&path, &serde_json::json!({"synthetic": true})).unwrap();
    let restored: serde_json::Value = super::super::read_json(&path).unwrap().unwrap();
    assert_eq!(restored, serde_json::json!({"synthetic": true}));
    assert_eq!(with_lock(&root.join("record.lock"), || Ok(42)).unwrap(), 42);
}
