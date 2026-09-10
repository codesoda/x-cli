use super::*;
use crate::credentials::database::tests::{PASSWORD, pair, schema};
use rusqlite::Connection;
use std::cell::Cell;

fn synthetic_profile(root: &Path, nested: bool) {
    let directory = if nested {
        root.join("Default/Network")
    } else {
        root.join("Default")
    };
    fs::create_dir_all(&directory).unwrap();
    let connection = Connection::open(directory.join("Cookies")).unwrap();
    schema(&connection);
    pair(&connection, ".x.com");
}

#[test]
fn consent_and_profile_validation_precede_all_access() {
    let directory = tempfile::tempdir().unwrap();
    let chrome = Chrome {
        root: directory.path().join("does-not-exist"),
    };
    for profile in ["Default", "../../escape", "/absolute", "Profile 1/Cookies"] {
        let error = chrome
            .load_with(profile, false, || panic!("key reader called"))
            .unwrap_err();
        assert_eq!(error.kind, Kind::ConsentRequired);
    }
    for profile in [
        "../Default",
        "Default/",
        "Profile -1",
        "Profile 01",
        "Profile ",
        "Profile 1\\evil",
        "Profile ١",
        "Default\0",
    ] {
        let error = chrome
            .load_with(profile, true, || panic!("key reader called"))
            .unwrap_err();
        assert_eq!(error.kind, Kind::InvalidInput);
    }
    assert_eq!(
        chrome.load("Default", false).unwrap_err().kind,
        Kind::ConsentRequired
    );
}

#[test]
fn discovery_reads_directory_metadata_only() {
    let directory = tempfile::tempdir().unwrap();
    for name in [
        "Default",
        "Profile 1",
        "Profile 20",
        "Guest Profile",
        "Profile 01",
    ] {
        fs::create_dir(directory.path().join(name)).unwrap();
    }
    // Not a SQLite DB; discovery must not try to parse it.
    fs::write(
        directory.path().join("Default/Cookies"),
        b"synthetic-not-a-db",
    )
    .unwrap();
    fs::write(directory.path().join("Profile 2"), b"not-a-directory").unwrap();
    let chrome = Chrome {
        root: directory.path().into(),
    };
    let profiles = chrome.discover().unwrap();
    assert_eq!(
        profiles
            .iter()
            .map(|p| p.directory.as_str())
            .collect::<Vec<_>>(),
        ["Default", "Profile 1", "Profile 20"]
    );
    assert_eq!(
        serde_json::to_string(&profiles[0]).unwrap(),
        "{\"directory\":\"Default\"}"
    );
}

#[test]
fn synthetic_load_and_keychain_denial_no_fallback() {
    for nested in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        synthetic_profile(directory.path(), nested);
        let chrome = Chrome {
            root: directory.path().into(),
        };
        assert!(
            chrome
                .load_with("Default", true, || Ok(Zeroizing::new(PASSWORD.to_vec())))
                .is_ok()
        );
        let called = Cell::new(0);
        let denied = chrome
            .load_with("Default", true, || {
                called.set(called.get() + 1);
                Err(Error::new(Kind::Permission, "Synthetic denial"))
            })
            .unwrap_err();
        assert_eq!(denied.kind, Kind::Permission);
        assert_eq!(called.get(), 1);
        assert_eq!(
            chrome
                .load_with("Default", true, || Ok(Zeroizing::new(vec![])))
                .unwrap_err()
                .kind,
            Kind::Unsupported
        );
    }
}

#[test]
fn multiple_databases_and_unknown_schema_do_not_request_key() {
    let directory = tempfile::tempdir().unwrap();
    synthetic_profile(directory.path(), false);
    let chrome = Chrome {
        root: directory.path().into(),
    };
    let connection = Connection::open(directory.path().join("Default/Cookies")).unwrap();
    connection
        .execute("UPDATE meta SET value='25'", [])
        .unwrap();
    assert_eq!(
        chrome
            .load_with("Default", true, || panic!("key reader called"))
            .unwrap_err()
            .kind,
        Kind::Unsupported
    );
    synthetic_profile(directory.path(), true);
    assert_eq!(
        chrome
            .load_with("Default", true, || panic!("key reader called"))
            .unwrap_err()
            .kind,
        Kind::Unavailable
    );
}

#[cfg(unix)]
#[test]
fn reject_profile_database_network_and_sidecar_symlinks() {
    use std::os::unix::fs::symlink;
    let outside = tempfile::tempdir().unwrap();
    for target in [
        "Default",
        "Default/Cookies",
        "Default/Network",
        "Default/Cookies-wal",
        "Default/Cookies-shm",
        "Default/Cookies-journal",
    ] {
        let directory = tempfile::tempdir().unwrap();
        if target == "Default" {
            symlink(outside.path(), directory.path().join(target)).unwrap();
        } else {
            synthetic_profile(directory.path(), false);
            let path = directory.path().join(target);
            if target == "Default/Cookies" {
                fs::remove_file(&path).unwrap();
            }
            symlink(outside.path(), path).unwrap();
        }
        let chrome = Chrome {
            root: directory.path().into(),
        };
        assert_eq!(
            chrome
                .load_with("Default", true, || panic!("key reader called"))
                .unwrap_err()
                .kind,
            Kind::Storage
        );
        if target == "Default" {
            assert!(chrome.discover().unwrap().is_empty());
        }
    }
}
