use super::*;

#[test]
fn list_posts_validate_account_id_and_route_before_external_access() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let connection = Config::update(&root, |config| {
        let connection = config.add(
            "Default".into(),
            Some("work".into()),
            crate::model::Identity {
                id: "123".into(),
                handle: "fixture".into(),
            },
        )?;
        config.set_default("work")?;
        Ok(connection)
    })
    .unwrap();
    for args in [
        vec!["lists", "posts", "456"],
        vec!["lists", "posts", "456", "--connection", &connection.id],
        vec!["lists", "posts", "456", "--account", ""],
        vec!["lists", "posts", "456", "--account", "work", "--cursor", ""],
    ] {
        assert_eq!(command(&root, &args).unwrap_err().kind, Kind::InvalidInput);
    }
    for id in [
        "",
        "0",
        "01",
        "-1",
        "1/2",
        "https://x.com/i/lists/456",
        "18446744073709551616",
    ] {
        // A negative positional argument is a clap error unless delimited; test
        // task validation with -- to avoid confusing parsing with ID validation.
        let args = ["lists", "posts", "--account", "work", "--", id];
        assert_eq!(command(&root, &args).unwrap_err().kind, Kind::InvalidInput);
    }
    assert_eq!(
        command(
            &root,
            &[
                "lists",
                "posts",
                "456",
                "--account",
                "work",
                "--backend",
                "fxtwitter"
            ]
        )
        .unwrap_err()
        .kind,
        Kind::Unsupported
    );
}

#[test]
fn list_members_validate_private_routing_and_user_cache_keys() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for args in [
        vec!["lists", "members", "list", "456"],
        vec![
            "lists",
            "members",
            "list",
            "456",
            "--connection",
            "connection-1",
        ],
        vec!["lists", "members", "list", "0", "--account", "work"],
        vec!["lists", "members", "list", "01", "--account", "work"],
        vec![
            "lists",
            "members",
            "list",
            "https://x.com/i/lists/456",
            "--account",
            "work",
        ],
        vec![
            "lists",
            "members",
            "list",
            "456",
            "--account",
            "work",
            "--cursor",
            "",
        ],
    ] {
        assert_eq!(command(&root, &args).unwrap_err().kind, Kind::InvalidInput);
    }
    assert_eq!(
        command(
            &root,
            &[
                "lists",
                "members",
                "list",
                "456",
                "--account",
                "work",
                "--backend",
                "fxtwitter"
            ]
        )
        .unwrap_err()
        .kind,
        Kind::Unsupported
    );
    let key = |argv: Vec<&str>| {
        let cli = Cli::try_parse_from(argv).unwrap();
        let (_, task) = task::Task::from_command(&cli.command).unwrap();
        (task.key(), task.expects_users())
    };
    let baseline = key(vec![
        "xcli",
        "lists",
        "members",
        "list",
        "456",
        "--account",
        "work",
    ]);
    assert!(baseline.1);
    for args in [
        vec![
            "xcli",
            "lists",
            "members",
            "list",
            "457",
            "--account",
            "work",
        ],
        vec![
            "xcli",
            "lists",
            "members",
            "list",
            "456",
            "--account",
            "work",
            "--cursor",
            "next",
        ],
        vec![
            "xcli",
            "lists",
            "members",
            "list",
            "456",
            "--account",
            "work",
            "--max-pages",
            "2",
        ],
        vec![
            "xcli",
            "lists",
            "members",
            "list",
            "456",
            "--account",
            "work",
            "--page-size",
            "5",
        ],
        vec!["xcli", "lists", "posts", "456", "--account", "work"],
    ] {
        assert_ne!(baseline.0, key(args).0);
    }
}

#[test]
fn list_posts_cli_excludes_ranked_and_mutation_commands() {
    let cli = Cli::try_parse_from(["xcli", "lists", "posts", "456", "--account", "work"]).unwrap();
    let (_, task) = task::Task::from_command(&cli.command).unwrap();
    assert!(task.requires_graphql());
    for args in [
        vec!["xcli", "lists", "create", "name", "--account", "work"],
        vec!["xcli", "lists", "delete", "456", "--account", "work"],
        vec![
            "xcli",
            "lists",
            "members",
            "add",
            "456",
            "someone",
            "--account",
            "work",
        ],
        vec!["xcli", "lists", "posts", "456", "--ranked"],
        vec!["xcli", "lists", "posts", "456", "--max-pages", "0"],
        vec!["xcli", "lists", "posts", "456", "--page-size", "101"],
        vec!["xcli", "lists", "posts", "456", "--refresh", "--no-cache"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

#[test]
fn list_posts_cache_keys_separate_lists_and_pagination() {
    let key = |args: &[&str]| {
        let cli = Cli::try_parse_from(std::iter::once("xcli").chain(args.iter().copied())).unwrap();
        task::Task::from_command(&cli.command).unwrap().1.key()
    };
    let baseline = key(&["lists", "posts", "456", "--account", "work"]);
    for args in [
        vec!["lists", "posts", "457", "--account", "work"],
        vec![
            "lists",
            "posts",
            "456",
            "--account",
            "work",
            "--cursor",
            "next",
        ],
        vec![
            "lists",
            "posts",
            "456",
            "--account",
            "work",
            "--max-pages",
            "2",
        ],
        vec![
            "lists",
            "posts",
            "456",
            "--account",
            "work",
            "--page-size",
            "5",
        ],
        vec!["likes", "list", "--account", "work"],
        vec!["bookmarks", "list", "--account", "work"],
        vec!["search", "456", "--account", "work"],
    ] {
        assert_ne!(baseline, key(&args));
    }
}
