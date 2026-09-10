use super::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

#[test]
fn size_caps_preserve_previous_file_and_reject_oversized_reads() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let path = root.join("data.json");
    write_json(&path, &42u64).unwrap();
    assert!(write_json(&path, &"x".repeat(MAX_JSON_BYTES)).is_err());
    assert_eq!(read_json::<u64>(&path).unwrap(), Some(42));
    let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(MAX_JSON_BYTES as u64 + 1).unwrap();
    assert!(read_json::<serde_json::Value>(&path).is_err());
    remove_file(&path).unwrap();
    write_json(&path, &7u64).unwrap();
    assert_eq!(read_json::<u64>(&path).unwrap(), Some(7));
}
#[test]
fn prune_unlinks_links_without_following_or_chmodding_outside_root() {
    let t = tempfile::tempdir().unwrap();
    let ancestor = t.path().canonicalize().unwrap();
    fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o755)).unwrap();
    let content = ancestor.join("data/content");
    write_json(&content.join("a"), &1).unwrap();
    write_json(&content.join("b"), &2).unwrap();
    write_json(&content.join("c"), &3).unwrap();
    prune_files(&content, Some("a"), 1).unwrap();
    assert_eq!(fs::read_dir(&content).unwrap().count(), 1);
    assert!(content.join("a").exists());
    let victim = ancestor.join("victim");
    fs::write(&victim, b"untouched").unwrap();
    fs::set_permissions(&victim, fs::Permissions::from_mode(0o644)).unwrap();
    symlink(&victim, content.join("link")).unwrap();
    symlink(&ancestor, content.join("directory-link")).unwrap();
    symlink(ancestor.join("missing"), content.join("dangling")).unwrap();
    prune_files(&content, None, 0).unwrap();
    assert_eq!(fs::read_dir(&content).unwrap().count(), 0);
    assert_eq!(fs::read(&victim).unwrap(), b"untouched");
    assert_eq!(
        fs::metadata(&victim).unwrap().permissions().mode() & 0o777,
        0o644
    );
    assert_eq!(
        fs::metadata(&ancestor).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(ancestor.join("data"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let link = ancestor.join("content-link");
    symlink(&content, &link).unwrap();
    assert!(prune_files(&link, None, 0).is_err());
    assert!(prune_files(&content, Some("../victim"), 1).is_err());
    fs::create_dir(content.join("unexpected-directory")).unwrap();
    assert!(prune_files(&content, None, 0).is_err());
    assert!(remove_file(&content.join("unexpected-directory")).is_err());
}
#[test]
fn roundtrip_missing_and_modes() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap().join("private/nested");
    let path = root.join("data.json");
    assert_eq!(read_json::<Vec<u64>>(&path).unwrap(), None);
    assert!(!root.exists());
    write_json(&path, &vec![1u64, 2]).unwrap();
    assert_eq!(read_json::<Vec<u64>>(&path).unwrap(), Some(vec![1, 2]));
    for dir in [&root, &root.parent().unwrap().to_path_buf()] {
        assert_eq!(
            fs::metadata(dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    write_json(&path, &vec![3u64]).unwrap();
    assert_eq!(read_json::<Vec<u64>>(&path).unwrap(), Some(vec![3]));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}
#[test]
fn atomic_replacement_preserves_open_snapshot_and_serialization_failure() {
    use std::io::Read;
    struct Fails;
    impl Serialize for Fails {
        fn serialize<S: serde::Serializer>(&self, _: S) -> std::result::Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("intentional test failure"))
        }
    }
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let path = root.join("data.json");
    write_json(&path, &vec![1u64, 2]).unwrap();
    let mut snapshot = fs::File::open(&path).unwrap();
    write_json(&path, &vec![3u64]).unwrap();
    let mut old = String::new();
    snapshot.read_to_string(&mut old).unwrap();
    assert_eq!(old, "[1,2]");
    assert!(write_json(&path, &Fails).is_err());
    assert_eq!(read_json::<Vec<u64>>(&path).unwrap(), Some(vec![3]));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}
#[test]
fn rejects_file_directory_dangling_symlinks_and_hardlinks() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let target = root.join("target.json");
    write_json(&target, &42).unwrap();
    let link = root.join("link.json");
    symlink(&target, &link).unwrap();
    assert!(read_json::<u64>(&link).is_err());
    assert!(write_json(&link, &1).is_err());
    fs::remove_file(&link).unwrap();
    symlink(root.join("absent"), &link).unwrap();
    assert!(read_json::<u64>(&link).is_err());
    assert!(write_json(&link, &1).is_err());
    let dirlink = root.join("dirlink");
    symlink(&root, &dirlink).unwrap();
    assert!(read_json::<u64>(&dirlink.join("target.json")).is_err());
    assert!(write_json(&dirlink.join("other.json"), &1).is_err());
    let hard = root.join("hard.json");
    fs::hard_link(&target, &hard).unwrap();
    assert!(read_json::<u64>(&hard).is_err());
    assert!(write_json(&hard, &1).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "42");
}
#[test]
fn rejects_corruption_and_nonfiles_and_repairs_permissions() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let path = root.join("data.json");
    fs::write(&path, b"not json").unwrap();
    assert!(read_json::<u64>(&path).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    write_json(&path, &7).unwrap();
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(read_json::<u64>(&root.join("../escape")).is_err());
    fs::create_dir(root.join("directory")).unwrap();
    assert!(read_json::<u64>(&root.join("directory")).is_err());
}
