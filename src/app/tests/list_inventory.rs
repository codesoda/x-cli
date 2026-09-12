use super::*;
use crate::model::{Identity, ListManagementSection::Pinned};

fn task(args: &[&str]) -> task::Task {
    let cli = Cli::try_parse_from(std::iter::once("xcli").chain(args.iter().copied())).unwrap();
    task::Task::from_command(&cli.command).unwrap().1
}

#[test]
fn list_inventory_requires_explicit_account_even_with_default_or_connection() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let (work, other) = Config::update(&root, |config| {
        let work = config.add(
            "Default".into(),
            Some("work".into()),
            Identity {
                id: "123".into(),
                handle: "fixture".into(),
            },
        )?;
        let other = config.add(
            "Profile 1".into(),
            Some("other".into()),
            Identity {
                id: "999".into(),
                handle: "other_fixture".into(),
            },
        )?;
        config.set_default("work")?;
        Ok((work, other))
    })
    .unwrap();
    for args in [
        vec!["lists", "list"],
        vec!["lists", "list", "--connection", &work.id],
        vec!["lists", "list", "--account", ""],
        vec!["lists", "list", "--account", "work", "--cursor", ""],
        vec![
            "lists",
            "list",
            "--account",
            "work",
            "--connection",
            &other.id,
        ],
        vec![
            "lists",
            "list",
            "--account",
            "work",
            "--connection",
            "missing",
        ],
    ] {
        assert_eq!(command(&root, &args).unwrap_err().kind, Kind::InvalidInput);
    }
    assert_eq!(
        command(
            &root,
            &[
                "lists",
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
    Config::update(&root, |config| {
        config.add("Profile 2".into(), None, work.identity.clone())?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        command(&root, &["lists", "list", "--account", "work"])
            .unwrap_err()
            .kind,
        Kind::InvalidInput
    );
}

#[test]
fn list_inventory_cli_is_bounded_viewer_only_without_mutations() {
    for account in ["work", "@fixture"] {
        let task = task(&[
            "lists",
            "list",
            "--account",
            account,
            "--connection",
            "connection-1",
        ]);
        assert!(task.requires_graphql() && task.expects_lists() && !task.expects_users());
    }
    for args in [
        vec!["xcli", "lists", "list", "some_user"],
        vec!["xcli", "lists", "list", "--user-id", "123"],
        vec!["xcli", "lists", "list", "--list-id", "456"],
        vec!["xcli", "lists", "list", "--max-pages", "0"],
        vec!["xcli", "lists", "list", "--max-pages", "21"],
        vec!["xcli", "lists", "list", "--page-size", "101"],
        vec!["xcli", "lists", "list", "--refresh", "--no-cache"],
        vec!["xcli", "lists", "create", "Fixture"],
        vec!["xcli", "lists", "update", "456"],
        vec!["xcli", "lists", "delete", "456"],
        vec!["xcli", "lists", "pin", "456"],
        vec!["xcli", "lists", "subscribe", "456"],
        vec!["xcli", "lists", "members", "add", "456", "fixture"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
    let cli = Cli::try_parse_from([
        "xcli",
        "lists",
        "list",
        "--account",
        "work",
        "--cursor",
        &"x".repeat(4097),
    ])
    .unwrap();
    assert!(task::Task::from_command(&cli.command).is_err());
}

#[test]
fn failed_inventory_output_is_not_cached() {
    let cli = Cli::try_parse_from(["xcli", "lists", "list", "--account", "work"]).unwrap();
    let (_, task) = task::Task::from_command(&cli.command).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::new(temp.path().canonicalize().unwrap().join("cache"));
    let mut output = Output::new("graphql", Some("123".into()));
    let mut list = list_metadata::fixture();
    list.management_sections.push(Pinned);
    output.lists = Some(vec![list]);
    output.pages = 1;
    output.request_failed = true;
    output.stop_reason = "request_failed".into();
    assert!(task.accepts_cached(&output)); // Shape is valid; fetch failure prohibits storage.
    let generation = cache.capture_generation().unwrap();
    save_cached(&cache, Some(&generation), &task.key(), &mut output).unwrap();
    assert!(
        cache
            .get("graphql", Some("123"), &task.key(), 300)
            .unwrap()
            .is_none()
    );
}

#[test]
fn list_inventory_cache_key_provenance_shape_and_account_isolation() {
    let inventory = task(&["lists", "list", "--account", "work"]);
    assert_eq!(
        inventory.key(),
        json!(["v1", "list_inventory", 1, 20, null]).to_string()
    );
    assert_eq!(
        inventory.key(),
        task(&["lists", "list", "--account", "other"]).key()
    );
    for args in [
        vec!["lists", "list", "--account", "work", "--cursor", "next"],
        vec!["lists", "list", "--account", "work", "--max-pages", "2"],
        vec!["lists", "list", "--account", "work", "--page-size", "5"],
        vec!["lists", "show", "456", "--account", "work"],
        vec!["lists", "posts", "456", "--account", "work"],
        vec!["lists", "members", "list", "456", "--account", "work"],
        vec!["following", "list", "--account", "work"],
        vec!["bookmarks", "list", "--account", "work"],
    ] {
        assert_ne!(inventory.key(), task(&args).key());
    }
    let mut output = Output::new("graphql", Some("123".into()));
    assert!(!inventory.accepts_cached(&output));
    output.lists = Some(vec![]);
    assert!(inventory.accepts_cached(&output));
    output.lists = Some(vec![list_metadata::fixture()]);
    assert!(!inventory.accepts_cached(&output));
    let metadata = task(&["lists", "show", "456", "--account", "work"]);
    assert!(metadata.accepts_cached(&output)); // Legacy single-list metadata stays valid.
    output.lists.as_mut().unwrap()[0].management_sections = vec![Pinned];
    assert!(inventory.accepts_cached(&output));
    let mut legacy = serde_json::to_value(&output).unwrap();
    legacy["lists"][0]
        .as_object_mut()
        .unwrap()
        .remove("management_sections");
    assert!(!inventory.accepts_cached(&serde_json::from_value(legacy).unwrap()));
    output.users = Some(vec![]);
    assert!(!inventory.accepts_cached(&output));
    output.users = None;
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::new(temp.path().canonicalize().unwrap().join("cache"));
    cache
        .put("graphql", Some("123"), &inventory.key(), &output)
        .unwrap();
    let hit = cache
        .get("graphql", Some("123"), &inventory.key(), 300)
        .unwrap()
        .unwrap();
    assert_eq!(hit.lists, output.lists);
    assert!(inventory.accepts_cached(&hit));
    let rendered = human(&serde_json::to_value(&hit).unwrap());
    assert!(rendered.contains("Management sections: pinned"));
    assert!(rendered.contains("https://x.com/i/lists/456"));
    for account in [None, Some("999")] {
        assert!(
            cache
                .get("graphql", account, &inventory.key(), 300)
                .unwrap()
                .is_none()
        );
    }
    assert!(
        cache
            .get("fxtwitter", None, &inventory.key(), 300)
            .unwrap()
            .is_none()
    );
    private_collection_cache_guards(&["lists", "list"]);
}
