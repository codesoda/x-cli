use super::*;

#[test]
fn relationships_require_account_and_reject_targets_writes_public_routing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let connection = Config::update(&root, |config| {
        let c = config.add(
            "Default".into(),
            Some("work".into()),
            crate::model::Identity {
                id: "123".into(),
                handle: "fixture".into(),
            },
        )?;
        config.set_default("work")?;
        Ok(c)
    })
    .unwrap();
    for op in ["following", "followers"] {
        for args in [
            vec![op, "list"],
            vec![op, "list", "--connection", &connection.id],
            vec![op, "list", "--account", ""],
            vec![op, "list", "--account", "work", "--cursor", ""],
        ] {
            assert_eq!(command(&root, &args).unwrap_err().kind, Kind::InvalidInput);
        }
        assert_eq!(
            command(
                &root,
                &[op, "list", "--account", "work", "--backend", "fxtwitter"]
            )
            .unwrap_err()
            .kind,
            Kind::Unsupported
        );
        for args in [
            vec!["xcli", op, "list", "someone", "--account", "work"],
            vec!["xcli", op, "list", "--user-id", "456"],
            vec!["xcli", op, "add", "someone", "--account", "work"],
            vec!["xcli", op, "remove", "someone", "--account", "work"],
            vec!["xcli", op, "list", "--max-pages", "0"],
            vec!["xcli", op, "list", "--page-size", "101"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
}
#[test]
fn relationship_cache_shape_rejects_legacy_missing_users() {
    let cli = Cli::try_parse_from(["xcli", "following", "list", "--account", "work"]).unwrap();
    let (_, task) = task::Task::from_command(&cli.command).unwrap();
    let mut output = Output::new("graphql", Some("123".into()));
    assert!(!task.accepts_cached(&output));
    output.users = Some(vec![]);
    assert!(task.accepts_cached(&output));
    let post_cli = Cli::try_parse_from(["xcli", "read", "20"]).unwrap();
    assert!(
        !task::Task::from_command(&post_cli.command)
            .unwrap()
            .1
            .accepts_cached(&output)
    );
}

#[test]
fn relationship_keys_distinguish_direction_and_pagination() {
    let key = |args: &[&str]| {
        let cli = Cli::try_parse_from(std::iter::once("xcli").chain(args.iter().copied())).unwrap();
        let (_, task) = task::Task::from_command(&cli.command).unwrap();
        assert!(task.requires_graphql());
        (task.key(), task.expects_users())
    };
    let base = key(&["following", "list", "--account", "work"]);
    assert!(base.1);
    for args in [
        vec!["followers", "list", "--account", "work"],
        vec!["following", "list", "--account", "work", "--cursor", "next"],
        vec!["following", "list", "--account", "work", "--max-pages", "2"],
        vec!["following", "list", "--account", "work", "--page-size", "5"],
        vec!["bookmarks", "list", "--account", "work"],
        vec!["likes", "list", "--account", "work"],
    ] {
        assert_ne!(base.0, key(&args).0);
    }
}
