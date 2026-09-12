use super::*;
use std::fs;

#[test]
fn invalidate_only_selected_scope_without_decoding_or_enumerating() {
    let (_dir, cache) = fixture();
    for (backend, account) in [
        ("graphql", Some("123")),
        ("graphql", Some("456")),
        ("fxtwitter", None),
        ("graphql", None),
        ("fxtwitter", Some("123")),
    ] {
        cache
            .put(
                backend,
                account,
                "key",
                &Output::new(backend, account.map(str::to_owned)),
            )
            .unwrap();
        cache.cooldown(backend, account, 120).unwrap();
    }
    let selected = cache.scope_path("graphql", Some("123")).unwrap();
    fs::write(&selected, b"malformed selected content").unwrap();
    let unrelated = cache.scope_path("graphql", Some("456")).unwrap();
    fs::write(&unrelated, b"malformed unrelated content").unwrap();
    // Enumeration/pruning would reject this unrelated directory.
    let directory = cache.root.join("content/unrelated-directory");
    fs::create_dir(&directory).unwrap();
    let mut preserved = vec![unrelated, cache.root.join("cooldowns.json")];
    for (backend, account) in [
        ("fxtwitter", None),
        ("graphql", None),
        ("fxtwitter", Some("123")),
    ] {
        preserved.push(cache.scope_path(backend, account).unwrap());
    }
    for name in ["cache.json", "config.json", "journal.json"] {
        let path = cache.root.join(name);
        fs::write(&path, b"synthetic untouched metadata").unwrap();
        preserved.push(path);
    }
    let before: Vec<_> = preserved.iter().map(|p| fs::read(p).unwrap()).collect();
    for _ in 0..2 {
        cache.invalidate_scope("graphql", Some("123")).unwrap();
        assert!(!selected.exists());
        assert!(directory.is_dir());
        for (path, bytes) in preserved.iter().zip(&before) {
            assert_eq!(&fs::read(path).unwrap(), bytes);
        }
    }
    assert_eq!(
        cache
            .check_cooldown("graphql", Some("123"))
            .unwrap_err()
            .kind,
        Kind::RateLimit
    );
}

#[test]
fn invalidate_missing_scope_is_harmless_and_does_not_create_content() {
    let (_dir, cache) = fixture();
    for _ in 0..2 {
        cache.invalidate_scope("graphql", Some("123")).unwrap();
        assert!(!cache.root.join("content").exists());
    }
}

#[cfg(unix)]
#[test]
fn invalidate_unlinks_links_without_touching_external_target_and_rejects_directory() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (dir, cache) = fixture();
    cache
        .put(
            "graphql",
            Some("123"),
            "key",
            &Output::new("graphql", Some("123".into())),
        )
        .unwrap();
    let selected = cache.scope_path("graphql", Some("123")).unwrap();
    fs::remove_file(&selected).unwrap();
    let outside = dir.path().canonicalize().unwrap().join("outside");
    fs::write(&outside, b"external synthetic marker").unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o640)).unwrap();
    for hardlink in [false, true] {
        if hardlink {
            fs::hard_link(&outside, &selected).unwrap();
        } else {
            symlink(&outside, &selected).unwrap();
        }
        cache.invalidate_scope("graphql", Some("123")).unwrap();
        assert!(fs::symlink_metadata(&selected).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"external synthetic marker");
        assert_eq!(
            fs::metadata(&outside).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
    fs::create_dir(&selected).unwrap();
    let child = selected.join("keep");
    fs::write(&child, b"directory child").unwrap();
    assert_eq!(
        cache
            .invalidate_scope("graphql", Some("123"))
            .unwrap_err()
            .kind,
        Kind::Storage
    );
    assert_eq!(fs::read(&child).unwrap(), b"directory child");
}

#[cfg(unix)]
#[test]
fn invalidate_rejects_symlink_parent_or_lock() {
    use std::os::unix::fs::symlink;
    for link_lock in [false, true] {
        let (dir, cache) = fixture();
        cache.invalidate_scope("graphql", Some("123")).unwrap();
        let outside = dir.path().canonicalize().unwrap().join("outside");
        if link_lock {
            fs::write(&outside, b"outside lock marker").unwrap();
            let lock = cache.root.join(".cache.lock");
            fs::remove_file(&lock).unwrap();
            symlink(&outside, &lock).unwrap();
        } else {
            fs::create_dir(&outside).unwrap();
            symlink(&outside, cache.root.join("content")).unwrap();
        }
        assert_eq!(
            cache
                .invalidate_scope("graphql", Some("123"))
                .unwrap_err()
                .kind,
            Kind::Storage
        );
    }
}
