use super::*;

fn access(b: Backend, account: Option<&str>) -> Access {
    Access {
        account: account.map(str::to_owned),
        connection: None,
        backend: b,
        no_cache: false,
        refresh: false,
        cache_ttl: 300,
    }
}
#[test]
fn routing() {
    assert_eq!(
        route(&access(Backend::Auto, None), false).unwrap(),
        Backend::Fxtwitter
    );
    assert_eq!(
        route(&access(Backend::Auto, Some("work")), false).unwrap(),
        Backend::Graphql
    );
    assert_eq!(
        route(&access(Backend::Auto, None), true).unwrap(),
        Backend::Graphql
    );
    assert!(route(&access(Backend::Fxtwitter, Some("work")), false).is_err());
    assert!(route(&access(Backend::Fxtwitter, None), true).is_err());
}
#[test]
fn cli_contract() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
    assert!(Cli::try_parse_from(["xcli", "read", "20", "--alias", "work"]).is_err());
    assert!(Cli::try_parse_from(["xcli", "search", "q", "--max-pages", "0"]).is_err());
    assert!(Cli::try_parse_from(["xcli", "likes", "add", "20"]).is_err());
}
#[test]
fn bookmarks_contract() {
    let cli = Cli::try_parse_from([
        "xcli",
        "bookmarks",
        "list",
        "--account",
        "work",
        "--max-pages",
        "2",
        "--page-size",
        "5",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Command::Bookmarks {
            command: BookmarkCommand::List { .. }
        }
    ));
    for args in [
        vec!["xcli", "bookmarks", "add", "20", "--account", "work"],
        vec!["xcli", "bookmarks", "remove", "20", "--account", "work"],
        vec!["xcli", "bookmarks", "list", "--max-pages", "0"],
        vec!["xcli", "bookmarks", "list", "--page-size", "101"],
        vec!["xcli", "bookmarks", "list", "--refresh", "--no-cache"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}
#[test]
fn update_contract() {
    for args in [
        ["xcli", "update"].as_slice(),
        &["xcli", "update", "--check"],
        &["xcli", "update", "-y"],
        &["xcli", "update", "--yes"],
    ] {
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(cli.command, Command::Update { .. }), "{args:?}");
    }
    // --check never installs, so combining it with --yes is contradictory.
    assert!(Cli::try_parse_from(["xcli", "update", "--check", "--yes"]).is_err());
    // Account/provider/pagination flags are rejected, not ignored.
    for flag in [
        "--account",
        "--connection",
        "--backend",
        "--max-pages",
        "--cursor",
    ] {
        assert!(
            Cli::try_parse_from(["xcli", "update", flag, "value"]).is_err(),
            "{flag}"
        );
    }
}

#[test]
fn cache_purge_contract() {
    for options in [
        vec![],
        vec!["--account", "work"],
        vec!["--account", "@fixture", "--connection", "connection-1"],
    ] {
        let mut args = vec!["xcli", "cache", "purge"];
        args.extend(options);
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(
            cli.command,
            Command::Cache {
                command: CacheCommand::Purge { .. }
            }
        ));
    }
    let error = Cli::try_parse_from(["xcli", "cache", "purge", "--connection", "connection-1"])
        .unwrap_err();
    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument
    );
    for options in [
        vec!["--backend", "graphql"],
        vec!["--cache-ttl", "300"],
        vec!["--refresh"],
        vec!["--no-cache"],
        vec!["--max-pages", "1"],
        vec!["--page-size", "20"],
        vec!["--cursor", "next"],
        vec!["--max-parents", "2"],
        vec!["--replies"],
    ] {
        for scoped in [false, true] {
            let mut args = vec!["xcli", "cache", "purge"];
            if scoped {
                args.extend(["--account", "work"]);
            }
            args.extend(options.iter().copied());
            assert_eq!(
                Cli::try_parse_from(args).unwrap_err().kind(),
                clap::error::ErrorKind::UnknownArgument
            );
        }
    }
}
