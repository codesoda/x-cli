use super::*;

mod walk;

fn id(file: &File) -> (u64, u64) {
    let meta = file.metadata().unwrap();
    (meta.dev(), meta.ino())
}
fn pair() -> (tempfile::TempDir, File, File) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::create_dir(root.join("child")).unwrap();
    let parent = File::open(&root).unwrap();
    let child = File::open(root.join("child")).unwrap();
    (temp, parent, child)
}

#[test]
fn existing_pairs_qualify_each_fd_before_sync_in_child_parent_order() {
    for child_readonly in [false, true] {
        for parent_readonly in [false, true] {
            let (_temp, parent, child) = pair();
            let events = std::cell::RefCell::new(Vec::new());
            sync_link_with(
                &parent,
                &child,
                false,
                |file| {
                    let child_fd = id(file) == id(&child);
                    events.borrow_mut().push(("flags", child_fd));
                    let readonly = if child_fd {
                        child_readonly
                    } else {
                        parent_readonly
                    };
                    Ok(if readonly { libc::ST_RDONLY } else { 0 })
                },
                |file| {
                    events.borrow_mut().push(("sync", id(file) == id(&child)));
                    Ok(())
                },
            )
            .unwrap();
            let mut expected = vec![("flags", true)];
            if !child_readonly {
                expected.push(("sync", true));
            }
            expected.push(("flags", false));
            if !parent_readonly {
                expected.push(("sync", false));
            }
            assert_eq!(*events.borrow(), expected);
        }
    }
}

#[test]
fn mount_query_failure_is_storage_error_and_stops_processing() {
    for fail_child in [true, false] {
        let (_temp, parent, child) = pair();
        let mut queries = Vec::new();
        let mut syncs = Vec::new();
        let error = sync_link_with(
            &parent,
            &child,
            false,
            |file| {
                let is_child = id(file) == id(&child);
                queries.push(is_child);
                if is_child == fail_child {
                    Err(storage())
                } else {
                    Ok(0)
                }
            },
            |file| {
                syncs.push(id(file) == id(&child));
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error.kind, crate::error::Kind::Storage);
        assert_eq!(
            queries,
            if fail_child {
                vec![true]
            } else {
                vec![true, false]
            }
        );
        assert_eq!(syncs, if fail_child { vec![] } else { vec![true] });
    }
}

#[test]
fn writable_sync_errors_are_never_swallowed_or_requalified() {
    for errno in [
        libc::ENOTSUP,
        libc::EACCES,
        libc::EPERM,
        libc::EIO,
        libc::EROFS,
    ] {
        for fail_child in [true, false] {
            let (_temp, parent, child) = pair();
            let mut queries = Vec::new();
            let mut syncs = Vec::new();
            let error = sync_link_with(
                &parent,
                &child,
                false,
                |file| {
                    queries.push(id(file) == id(&child));
                    Ok(0)
                },
                |file| {
                    let is_child = id(file) == id(&child);
                    syncs.push(is_child);
                    if is_child == fail_child {
                        Err(io::Error::from_raw_os_error(errno))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert_eq!(error.kind, crate::error::Kind::Storage);
            let expected = if fail_child {
                vec![true]
            } else {
                vec![true, false]
            };
            assert_eq!(queries, expected);
            assert_eq!(syncs, expected);
        }
    }
}

#[test]
fn new_pairs_never_query_readonly_flags_and_propagate_errors() {
    for failure in [None, Some(true), Some(false)] {
        let (_temp, parent, child) = pair();
        let mut syncs = Vec::new();
        let result = sync_link_with(
            &parent,
            &child,
            true,
            |_| panic!("new pair must never qualify readonly exemption"),
            |file| {
                let is_child = id(file) == id(&child);
                syncs.push(is_child);
                if failure == Some(is_child) {
                    Err(io::Error::from_raw_os_error(libc::EROFS))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.is_err(), failure.is_some());
        assert_eq!(
            syncs,
            if failure == Some(true) {
                vec![true]
            } else {
                vec![true, false]
            }
        );
    }
}

#[test]
fn actual_mount_flags_and_standard_sync_work_on_temporary_descriptors() {
    let (_temp, parent, child) = pair();
    // Only synthetic temporary descriptors reach the actual syscall helpers.
    assert_eq!(mount_flags(&child).unwrap() & libc::ST_RDONLY, 0);
    assert_eq!(mount_flags(&parent).unwrap() & libc::ST_RDONLY, 0);
    sync_directory_link(&parent, &child, false).unwrap();
    sync_directory_link(&parent, &child, true).unwrap();
}
