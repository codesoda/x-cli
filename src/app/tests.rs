use super::*;
use clap::Parser;

struct NoExternalAccess;
impl Transport for NoExternalAccess {
    fn get(&self, _: crate::transport::Request) -> Result<crate::transport::Response> {
        panic!("unexpected network access")
    }
}
impl CredentialProvider for NoExternalAccess {
    fn load(&self, _: &str, _: bool) -> Result<crate::credentials::Session> {
        panic!("unexpected credential access")
    }
}
fn command(root: &std::path::Path, args: &[&str]) -> Result<Value> {
    let mut argv = vec!["xcli", "--data-dir", root.to_str().unwrap()];
    argv.extend_from_slice(args);
    let cli = Cli::try_parse_from(argv).unwrap();
    execute(&cli, &NoExternalAccess, &NoExternalAccess, || {
        panic!("unexpected profile discovery")
    })
}

#[test]
fn validation_precedes_external_access() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for args in [
        vec!["search", ""],
        vec!["search", "query", "--cursor", ""],
        vec!["user", "posts", "invalid/handle"],
        vec!["bookmarks", "list"],
        vec!["bookmarks", "list", "--connection", "connection-1"],
        vec!["bookmarks", "list", "--account", ""],
        vec!["bookmarks", "list", "--account", "work", "--cursor", ""],
        vec![
            "auth",
            "add",
            "--profile",
            "Default",
            "--alias",
            "@invalid",
            "--consent",
        ],
    ] {
        assert_eq!(command(&root, &args).unwrap_err().kind, Kind::InvalidInput);
    }
}

#[test]
fn bookmarks_reject_public_provider_before_external_access() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let error = command(
        &root,
        &[
            "bookmarks",
            "list",
            "--account",
            "work",
            "--backend",
            "fxtwitter",
        ],
    )
    .unwrap_err();
    assert_eq!(error.kind, Kind::Unsupported);
}

#[test]
fn bookmark_cache_keys_separate_operations_and_pagination() {
    let key = |args: &[&str]| {
        let mut argv = vec!["xcli"];
        argv.extend_from_slice(args);
        let cli = Cli::try_parse_from(argv).unwrap();
        let (_, task) = task::Task::from_command(&cli.command).unwrap();
        assert!(task.requires_graphql());
        task.key()
    };
    let bookmarks = key(&["bookmarks", "list", "--account", "work"]);
    for args in [
        vec!["search", "bookmarks"],
        vec!["user", "posts", "bookmarks"],
        vec!["bookmarks", "list", "--account", "work", "--cursor", "next"],
        vec!["bookmarks", "list", "--account", "work", "--page-size", "5"],
        vec!["bookmarks", "list", "--account", "work", "--max-pages", "2"],
    ] {
        assert_ne!(bookmarks, key(&args));
    }
    // Account isolation is provided by the enclosing stable-ID cache scope,
    // not aliases embedded in an operation key.
    assert_eq!(bookmarks, key(&["bookmarks", "list", "--account", "other"]));
}

#[test]
fn local_account_commands_preserve_registration_behavior() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let connection = Config::update(&root, |config| {
        config.add(
            "Default".into(),
            Some("work".into()),
            crate::model::Identity {
                id: "123".into(),
                handle: "fixture".into(),
            },
        )
    })
    .unwrap();
    let listed = command(&root, &["auth", "list"]).unwrap();
    assert_eq!(listed["connections"][0]["id"], connection.id);
    command(
        &root,
        &["auth", "rename", &connection.id, "--alias", "office"],
    )
    .unwrap();
    command(&root, &["auth", "prefer", &connection.id]).unwrap();
    command(&root, &["auth", "default", "office"]).unwrap();
    let config = Config::load(&root).unwrap();
    assert_eq!(
        config.resolve(None, None).unwrap().alias.as_deref(),
        Some("office")
    );
    assert!(config.connections[0].preferred);
    command(&root, &["auth", "remove", &connection.id]).unwrap();
    assert!(Config::load(&root).unwrap().connections.is_empty());
}

#[test]
fn bookmark_cache_never_bypasses_session_loading_or_cooldown() {
    struct Expired;
    impl CredentialProvider for Expired {
        fn load(&self, profile: &str, consent: bool) -> Result<crate::credentials::Session> {
            assert_eq!(profile, "Default");
            assert!(consent);
            Err(Error::new(
                Kind::Authentication,
                "Synthetic expired session",
            ))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    Config::update(&root, |config| {
        config.add(
            "Default".into(),
            Some("work".into()),
            crate::model::Identity {
                id: "123".into(),
                handle: "fixture".into(),
            },
        )
    })
    .unwrap();
    let cli = Cli::try_parse_from([
        "xcli",
        "--data-dir",
        root.to_str().unwrap(),
        "bookmarks",
        "list",
        "--account",
        "work",
    ])
    .unwrap();
    let (_, task) = task::Task::from_command(&cli.command).unwrap();
    let cache = Cache::new(root.join("cache"));
    cache
        .put(
            "graphql",
            Some("123"),
            &task.key(),
            &Output::new("graphql", Some("123".into())),
        )
        .unwrap();
    let error = execute(&cli, &NoExternalAccess, &Expired, || {
        panic!("unexpected discovery")
    })
    .unwrap_err();
    assert_eq!(error.kind, Kind::Authentication);
    cache.cooldown("graphql", Some("123"), 60).unwrap();
    let error = execute(&cli, &NoExternalAccess, &NoExternalAccess, || {
        panic!("unexpected discovery")
    })
    .unwrap_err();
    assert_eq!(error.kind, Kind::RateLimit);
}

#[test]
fn rate_limit_stops_following_request() {
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::new(temp.path().canonicalize().unwrap());
    let first: Result<()> = rate_call(&cache, "graphql", Some("123"), || {
        Err(Error::new(Kind::RateLimit, "Synthetic rate limit"))
    });
    assert_eq!(first.unwrap_err().kind, Kind::RateLimit);
    let next: Result<()> = rate_call(&cache, "graphql", Some("123"), || {
        panic!("request during cooldown")
    });
    assert_eq!(next.unwrap_err().kind, Kind::RateLimit);
}
