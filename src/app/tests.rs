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
