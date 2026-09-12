use super::*;
use crate::model::Identity;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

type Snapshot = BTreeMap<PathBuf, Vec<u8>>;
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    cache: Cache,
    selected: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut config = Config::default();
        for (profile, alias, id, handle) in [
            ("Default", "work", "123", "fixture"),
            ("Profile 1", "personal", "456", "other"),
            ("Profile 2", "duplicate", "123", "fixture"),
        ] {
            config
                .add(
                    profile.into(),
                    Some(alias.into()),
                    Identity {
                        id: id.into(),
                        handle: handle.into(),
                    },
                )
                .unwrap();
        }
        config.save(&root).unwrap();
        let cache = Cache::new(root.join("cache"));
        cache
            .put(
                "graphql",
                Some("123"),
                "key",
                &Output::new("graphql", Some("123".into())),
            )
            .unwrap();
        let selected = fs::read_dir(root.join("cache/content"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        for (backend, account) in [("graphql", Some("456")), ("fxtwitter", None)] {
            cache
                .put(
                    backend,
                    account,
                    "key",
                    &Output::new(backend, account.map(str::to_owned)),
                )
                .unwrap();
        }
        for (backend, account) in [
            ("graphql", Some("123")),
            ("graphql", Some("456")),
            ("fxtwitter", None),
        ] {
            cache.cooldown(backend, account, 120).unwrap();
        }
        for file in ["cache/cache.json", "journal.json"] {
            fs::write(root.join(file), b"synthetic untouched metadata").unwrap();
        }
        Self {
            _temp: temp,
            root,
            cache,
            selected,
        }
    }
    fn snapshot(&self) -> Snapshot {
        let mut paths: Vec<_> = fs::read_dir(self.root.join("cache/content"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        for file in [
            "config.json",
            "cache/cooldowns.json",
            "cache/cache.json",
            "journal.json",
        ] {
            paths.push(self.root.join(file));
        }
        paths
            .into_iter()
            .map(|p| {
                let bytes = fs::read(&p).unwrap();
                (p, bytes)
            })
            .collect()
    }
}
fn scoped_result() -> Value {
    json!({"purged":true,"backend":"graphql","account_id":"123","rate_limit_cooldowns_preserved":true})
}
fn purge(root: &Path, selector: &str) -> Result<Value> {
    command(root, &["cache", "purge", "--account", selector])
}

#[test]
fn scoped_purge_uses_only_local_identity_and_preserves_other_scope_bytes() {
    for selector in ["work", "@FIXTURE", "duplicate"] {
        let f = Fixture::new();
        // Same-account profiles without preference remain ambiguous for reads.
        assert!(
            Config::load(&f.root)
                .unwrap()
                .resolve(Some(selector), None)
                .is_err()
        );
        let mut before = f.snapshot();
        before.remove(&f.selected).unwrap();
        for _ in 0..2 {
            assert_eq!(purge(&f.root, selector).unwrap(), scoped_result());
            assert_eq!(f.snapshot(), before);
            assert!(
                f.cache
                    .get("graphql", Some("123"), "key", 300)
                    .unwrap()
                    .is_none()
            );
            assert!(
                f.cache
                    .get("graphql", Some("456"), "key", 300)
                    .unwrap()
                    .is_some()
            );
            assert!(
                f.cache
                    .get("fxtwitter", None, "key", 300)
                    .unwrap()
                    .is_some()
            );
            assert_eq!(
                f.cache
                    .check_cooldown("graphql", Some("123"))
                    .unwrap_err()
                    .kind,
                Kind::RateLimit
            );
        }
    }
}

#[test]
fn scoped_purge_handles_corrupt_content_without_external_access() {
    let f = Fixture::new();
    for entry in fs::read_dir(f.root.join("cache/content")).unwrap() {
        fs::write(entry.unwrap().path(), b"malformed synthetic cache").unwrap();
    }
    let mut before = f.snapshot();
    before.remove(&f.selected).unwrap();
    assert_eq!(purge(&f.root, "work").unwrap(), scoped_result());
    assert_eq!(f.snapshot(), before);
}

#[test]
fn invalid_selectors_or_mismatched_connections_delete_nothing() {
    let f = Fixture::new();
    let before = f.snapshot();
    for selector in ["", "unknown", "@unknown", "@", "fixture"] {
        assert_eq!(
            purge(&f.root, selector).unwrap_err().kind,
            Kind::InvalidInput
        );
        assert_eq!(f.snapshot(), before);
    }
    for (selector, connection) in [
        ("work", "connection-2"),
        ("@fixture", "connection-2"),
        ("work", "missing"),
        ("", "connection-1"),
    ] {
        assert_eq!(
            command(
                &f.root,
                &[
                    "cache",
                    "purge",
                    "--account",
                    selector,
                    "--connection",
                    connection
                ]
            )
            .unwrap_err()
            .kind,
            Kind::InvalidInput
        );
        assert_eq!(f.snapshot(), before);
    }
}

#[test]
fn constructed_connection_only_command_cannot_trigger_global_purge() {
    let f = Fixture::new();
    let before = f.snapshot();
    let cli = Cli {
        command: Command::Cache {
            command: CacheCommand::Purge {
                account: None,
                connection: Some("connection-1".into()),
            },
        },
        data_dir: Some(f.root.clone()),
        human: false,
    };
    let error = execute(&cli, &NoExternalAccess, &NoExternalAccess, || {
        panic!("unexpected profile discovery")
    })
    .unwrap_err();
    assert_eq!(error.kind, Kind::InvalidInput);
    assert_eq!(f.snapshot(), before);
}

#[test]
fn reused_handle_requires_matching_explicit_connection() {
    let f = Fixture::new();
    Config::update(&f.root, |c| {
        c.connections[1].identity.handle = "fixture".into();
        Ok(())
    })
    .unwrap();
    let mut before = f.snapshot();
    assert_eq!(
        purge(&f.root, "@fixture").unwrap_err().kind,
        Kind::InvalidInput
    );
    assert_eq!(f.snapshot(), before);
    assert_eq!(
        command(
            &f.root,
            &[
                "cache",
                "purge",
                "--account",
                "@fixture",
                "--connection",
                "connection-1"
            ]
        )
        .unwrap(),
        scoped_result()
    );
    before.remove(&f.selected).unwrap();
    assert_eq!(f.snapshot(), before);
}

#[test]
fn global_purge_ignores_corrupt_config_and_preserves_cooldowns_and_result() {
    let f = Fixture::new();
    fs::write(f.root.join("config.json"), b"malformed synthetic config").unwrap();
    let before = f.snapshot();
    assert_eq!(purge(&f.root, "work").unwrap_err().kind, Kind::Storage);
    assert_eq!(f.snapshot(), before);
    for _ in 0..2 {
        assert_eq!(
            command(&f.root, &["cache", "purge"]).unwrap(),
            json!({"purged":true,"rate_limit_cooldowns_preserved":true})
        );
        assert_eq!(
            fs::read_dir(f.root.join("cache/content")).unwrap().count(),
            0
        );
        assert!(!f.root.join("cache/cache.json").exists());
        for name in ["config.json", "cache/cooldowns.json", "journal.json"] {
            let path = f.root.join(name);
            assert_eq!(fs::read(&path).unwrap(), before[&path]);
        }
        assert_eq!(
            f.cache
                .check_cooldown("graphql", Some("123"))
                .unwrap_err()
                .kind,
            Kind::RateLimit
        );
    }
}

#[test]
fn dispatched_purges_fence_managed_writes_and_preserve_generation_metadata() {
    for global in [false, true] {
        let f = Fixture::new();
        let token = f.cache.capture_generation().unwrap();
        if global {
            command(&f.root, &["cache", "purge"]).unwrap();
        } else {
            purge(&f.root, "work").unwrap();
        }
        assert_eq!(
            f.cache
                .put_if_generation(
                    &token,
                    "graphql",
                    Some("123"),
                    "key",
                    &Output::new("graphql", Some("123".into()))
                )
                .unwrap(),
            ConditionalPut::GenerationChanged
        );
        let metadata: Value = crate::state::read_json(&f.root.join("cache/generation.json"))
            .unwrap()
            .unwrap();
        assert_eq!(metadata, json!({"version":1,"generation":1}));
        assert!(!f.selected.exists());
    }
}

#[test]
fn corrupt_generation_blocks_both_purge_commands_before_any_content_deletion() {
    let f = Fixture::new();
    let path = f.root.join("cache/generation.json");
    fs::write(&path, b"invalid synthetic metadata").unwrap();
    let before = f.snapshot();
    assert_eq!(purge(&f.root, "work").unwrap_err().kind, Kind::Storage);
    assert_eq!(f.snapshot(), before);
    // Global purge ignores corrupt config, but must not reset a corrupt epoch.
    fs::write(f.root.join("config.json"), b"invalid synthetic config").unwrap();
    let before = f.snapshot();
    assert_eq!(
        command(&f.root, &["cache", "purge"]).unwrap_err().kind,
        Kind::Storage
    );
    assert_eq!(f.snapshot(), before);
    assert_eq!(fs::read(path).unwrap(), b"invalid synthetic metadata");
}

#[test]
fn missing_config_is_not_an_account_or_required_for_global_purge() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    assert_eq!(purge(&root, "work").unwrap_err().kind, Kind::InvalidInput);
    assert!(!root.join("cache").exists());
    assert_eq!(
        command(&root, &["cache", "purge"]).unwrap(),
        json!({"purged":true,"rate_limit_cooldowns_preserved":true})
    );
    assert!(!root.join("config.json").exists());
}
