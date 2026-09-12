use super::*;

#[test]
fn own_likes_require_explicit_account_even_with_default() {
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
        vec!["likes", "list"],
        vec!["likes", "list", "--connection", &connection.id],
        vec!["likes", "list", "--account", ""],
        vec!["likes", "list", "--account", "work", "--cursor", ""],
    ] {
        assert_eq!(command(&root, &args).unwrap_err().kind, Kind::InvalidInput);
    }
    assert_eq!(
        command(
            &root,
            &[
                "likes",
                "list",
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
fn own_likes_have_no_target_user_or_write_commands() {
    use crate::cli::{Command, LikesCommand};
    let cli = Cli::try_parse_from(["xcli", "likes", "list", "--account", "work"]).unwrap();
    assert!(matches!(
        cli.command,
        Command::Likes {
            command: LikesCommand::List { .. }
        }
    ));
    for args in [
        vec!["xcli", "likes", "list", "someone", "--account", "work"],
        vec![
            "xcli",
            "likes",
            "list",
            "--user-id",
            "456",
            "--account",
            "work",
        ],
        vec!["xcli", "likes", "add", "20", "--account", "work"],
        vec!["xcli", "likes", "remove", "20", "--account", "work"],
        vec!["xcli", "likes", "list", "--max-pages", "0"],
        vec!["xcli", "likes", "list", "--page-size", "101"],
        vec!["xcli", "likes", "list", "--refresh", "--no-cache"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

#[test]
fn own_likes_keys_separate_operations_and_page_bounds() {
    let key = |args: &[&str]| {
        let cli = Cli::try_parse_from(std::iter::once("xcli").chain(args.iter().copied())).unwrap();
        let (_, task) = task::Task::from_command(&cli.command).unwrap();
        assert!(task.requires_graphql());
        task.key()
    };
    let likes = key(&["likes", "list", "--account", "work"]);
    for args in [
        vec!["bookmarks", "list", "--account", "work"],
        vec!["user", "posts", "fixture"],
        vec!["search", "likes"],
        vec!["likes", "list", "--account", "work", "--cursor", "next"],
        vec!["likes", "list", "--account", "work", "--max-pages", "2"],
        vec!["likes", "list", "--account", "work", "--page-size", "5"],
    ] {
        assert_ne!(likes, key(&args));
    }
}
