use super::*;

fn chain(path: &Path) -> Vec<(u64, u64)> {
    let mut paths: Vec<_> = path.ancestors().collect();
    paths.reverse();
    paths
        .into_iter()
        .map(|p| id(&File::open(p).unwrap()))
        .collect()
}

#[test]
fn every_link_is_processed_before_descent_and_on_all_existing_retry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let first = root.join("first");
    let second = first.join("second");
    let path = second.join("record.json");
    let ancestors = chain(&root);
    for retry in [false, true] {
        let mut links = Vec::new();
        parent_with_sync(&path, true, |parent, child, created| {
            links.push((id(parent), id(child), created));
            assert!(!path.exists());
            if first.exists() && id(child) == id(&File::open(&first).unwrap()) {
                assert_eq!(created, !retry);
                assert_eq!(second.exists(), retry);
                assert_eq!(child.metadata().unwrap().mode() & 0o777, 0o700);
            } else if !retry && !created {
                assert!(!first.exists());
            }
            Ok(())
        })
        .unwrap();
        let mut expected = ancestors.clone();
        expected.push(id(&File::open(&first).unwrap()));
        expected.push(id(&File::open(&second).unwrap()));
        let expected: Vec<_> = expected
            .windows(2)
            .enumerate()
            .map(|(i, w)| (w[0], w[1], !retry && i >= ancestors.len() - 1))
            .collect();
        assert_eq!(links, expected);
    }
}

#[test]
fn noncreating_existing_and_missing_walks_never_sync_or_create() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for exists in [false, true] {
        let first = root.join("first");
        if exists {
            std::fs::create_dir(&first).unwrap();
        }
        let result = parent_with_ops(
            &first.join("record.json"),
            false,
            |_, _, _| panic!("noncreating walk synced a link"),
            |_, _| panic!("noncreating walk attempted mkdir"),
        )
        .unwrap();
        assert_eq!(result.is_some(), exists);
        assert_eq!(first.exists(), exists);
        assert!(!first.join("record.json").exists());
    }
}

#[test]
fn existing_intermediate_modes_stay_unchanged_and_final_mode_is_repaired() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let first = root.join("first");
    let second = first.join("second");
    std::fs::create_dir_all(&second).unwrap();
    for create in [false, true] {
        for path in [&root, &first, &second] {
            std::fs::set_permissions(path, Permissions::from_mode(0o755)).unwrap();
        }
        parent_with_sync(&second.join("record"), create, |_, _, made| {
            assert!(!made);
            Ok(())
        })
        .unwrap();
        for path in [&root, &first] {
            assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o777, 0o755);
        }
        assert_eq!(std::fs::metadata(&second).unwrap().mode() & 0o777, 0o700);
    }
}

#[test]
fn eexist_race_is_existing_and_never_chmods_intermediate() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let first = root.join("first");
    let second = first.join("second");
    let mut creations = 0;
    let mut raced = false;
    parent_with_ops(
        &second.join("record"),
        true,
        |_, child, created| {
            if first.exists() && id(child) == id(&File::open(&first).unwrap()) {
                assert!(!created);
                assert_eq!(child.metadata().unwrap().mode() & 0o777, 0o755);
                assert!(!second.exists());
                raced = true;
            }
            Ok(())
        },
        |dir, part| {
            creations += 1;
            create_directory(dir, part)?;
            if part.to_bytes() == b"first" {
                // Deterministic competitor wins after open returned ENOENT.
                let winner = open(dir, part, DIRECTORY).unwrap();
                winner
                    .set_permissions(Permissions::from_mode(0o755))
                    .unwrap();
                Err(io::Error::from_raw_os_error(libc::EEXIST))
            } else {
                Ok(())
            }
        },
    )
    .unwrap();
    assert!(raced);
    assert_eq!(creations, 2);
    assert_eq!(std::fs::metadata(&first).unwrap().mode() & 0o777, 0o755);
    assert_eq!(std::fs::metadata(&second).unwrap().mode() & 0o777, 0o700);
}

#[test]
fn readonly_root_exemption_does_not_skip_writable_descendants() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let filesystem_root = id(&File::open("/").unwrap());
    let ancestors = chain(&root);
    let mut syncs = Vec::new();
    parent_with_sync(&root.join("record"), true, |parent, child, created| {
        assert!(!created);
        sync_link_with(
            parent,
            child,
            created,
            |fd| {
                Ok(if id(fd) == filesystem_root {
                    libc::ST_RDONLY
                } else {
                    0
                })
            },
            |fd| {
                syncs.push(id(fd));
                Ok(())
            },
        )
    })
    .unwrap();
    let expected: Vec<_> = ancestors
        .windows(2)
        .flat_map(|w| [w[1], w[0]])
        .filter(|fd| *fd != filesystem_root)
        .collect();
    assert_eq!(syncs, expected);
    assert!(syncs.contains(&id(&File::open(&root).unwrap())));
}
