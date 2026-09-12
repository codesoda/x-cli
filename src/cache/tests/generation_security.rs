use super::*;

#[cfg(unix)]
#[test]
fn generation_is_private_and_rejects_links_without_touching_content_or_targets() {
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };
    for hardlink in [false, true] {
        let (dir, cache) = fixture();
        cache.purge().unwrap();
        let metadata = cache.root.join("generation.json");
        assert_eq!(
            fs::metadata(&metadata).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let token = cache.capture_generation().unwrap();
        cache
            .put("one", None, "key", &Output::new("one", None))
            .unwrap();
        let scope = cache.scope_path("one", None).unwrap();
        let content = fs::read(&scope).unwrap();
        let outside = dir.path().canonicalize().unwrap().join("outside");
        fs::write(&outside, br#"{"version":1,"generation":1}"#).unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o640)).unwrap();
        fs::remove_file(&metadata).unwrap();
        if hardlink {
            fs::hard_link(&outside, &metadata).unwrap();
        } else {
            symlink(&outside, &metadata).unwrap();
        }
        assert_eq!(cache.capture_generation().unwrap_err().kind, Kind::Storage);
        assert_eq!(
            cache
                .put_if_generation(&token, "one", None, "key", &Output::new("one", None))
                .unwrap_err()
                .kind,
            Kind::Storage
        );
        assert_eq!(cache.purge().unwrap_err().kind, Kind::Storage);
        assert_eq!(
            cache.invalidate_scope("one", None).unwrap_err().kind,
            Kind::Storage
        );
        assert_eq!(fs::read(&scope).unwrap(), content);
        assert_eq!(
            fs::read(&outside).unwrap(),
            br#"{"version":1,"generation":1}"#
        );
        assert_eq!(
            fs::metadata(&outside).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}
