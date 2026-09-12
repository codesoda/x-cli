use super::*;
use crate::model::{Identity, ListInfo, ListVisibility, Post};

pub(super) fn fixture() -> ListInfo {
    ListInfo {
        id: "456".into(),
        name: "Synthetic list".into(),
        description: Some("Fixture description".into()),
        visibility: Some(ListVisibility::Private),
        owner: Some(Identity {
            id: "789".into(),
            handle: "owner_fixture".into(),
        }),
        subscribed: None,
        pinned: None,
        is_member: None,
        management_sections: vec![],
        url: "https://x.com/i/lists/456".into(),
    }
}

#[test]
fn list_metadata_validates_explicit_account_id_backend_and_connection_before_access() {
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
        vec!["lists", "show", "456"],
        vec!["lists", "show", "456", "--connection", &work.id],
        vec!["lists", "show", "456", "--account", ""],
        vec![
            "lists",
            "show",
            "456",
            "--account",
            "work",
            "--connection",
            &other.id,
        ],
        vec![
            "lists",
            "show",
            "456",
            "--account",
            "work",
            "--connection",
            "missing",
        ],
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
        assert_eq!(
            command(&root, &["lists", "show", "--account", "work", "--", id])
                .unwrap_err()
                .kind,
            Kind::InvalidInput
        );
    }
    assert_eq!(
        command(
            &root,
            &[
                "lists",
                "show",
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
    Config::update(&root, |config| {
        config.add("Profile 2".into(), None, work.identity.clone())?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        command(&root, &["lists", "show", "456", "--account", "work"])
            .unwrap_err()
            .kind,
        Kind::InvalidInput
    );
}

#[test]
fn list_metadata_cli_has_no_paging_discovery_or_mutations() {
    let cli = Cli::try_parse_from([
        "xcli",
        "lists",
        "show",
        "456",
        "--account",
        "work",
        "--connection",
        "connection-1",
    ])
    .unwrap();
    let (_, task) = task::Task::from_command(&cli.command).unwrap();
    assert!(task.requires_graphql() && task.expects_lists() && !task.expects_users());
    for args in [
        vec!["xcli", "lists", "show", "456", "--max-pages", "1"],
        vec!["xcli", "lists", "show", "456", "--page-size", "1"],
        vec!["xcli", "lists", "show", "456", "--cursor", "next"],
        vec!["xcli", "lists", "show", "456", "--refresh", "--no-cache"],
        vec!["xcli", "lists", "show", "456", "--name", "updated"],
        vec!["xcli", "lists", "create", "Fixture"],
        vec!["xcli", "lists", "update", "456"],
        vec!["xcli", "lists", "delete", "456"],
        vec!["xcli", "lists", "members", "add", "456", "fixture"],
        vec!["xcli", "lists", "members", "remove", "456", "fixture"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

fn task(args: &[&str]) -> task::Task {
    let cli = Cli::try_parse_from(std::iter::once("xcli").chain(args.iter().copied())).unwrap();
    task::Task::from_command(&cli.command).unwrap().1
}

#[test]
fn list_metadata_cache_key_and_shape_are_legacy_safe() {
    let metadata = task(&["lists", "show", "456", "--account", "work"]);
    assert_eq!(
        metadata.key(),
        json!(["v1", "list_metadata", "456"]).to_string()
    );
    assert_eq!(
        metadata.key(),
        task(&["lists", "show", "456", "--account", "other"]).key()
    );
    for args in [
        vec!["lists", "show", "457", "--account", "work"],
        vec!["lists", "posts", "456", "--account", "work"],
        vec!["lists", "members", "list", "456", "--account", "work"],
        vec!["read", "456"],
    ] {
        assert_ne!(metadata.key(), task(&args).key());
    }
    let users = task(&["lists", "members", "list", "456", "--account", "work"]);
    let posts = task(&["read", "456"]);
    for (has_users, has_lists) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut output = Output::new("graphql", Some("123".into()));
        output.users = has_users.then(Vec::new);
        output.lists = has_lists.then(|| vec![fixture()]);
        assert_eq!(metadata.accepts_cached(&output), has_lists && !has_users);
        assert_eq!(users.accepts_cached(&output), has_users && !has_lists);
        assert_eq!(posts.accepts_cached(&output), !has_users && !has_lists);
        output.posts.push(Post {
            id: "20".into(),
            author: Identity {
                id: "12".into(),
                handle: "fixture".into(),
            },
            text: "Synthetic post".into(),
            created_at: None,
            parent_id: None,
            parent_known: true,
            url: "https://x.com/fixture/status/20".into(),
        });
        assert!(!metadata.accepts_cached(&output));
        assert!(!users.accepts_cached(&output));
    }
    private_collection_cache_guards(&["lists", "show", "456"]);
}

#[test]
fn list_metadata_json_human_and_private_cache_roundtrip() {
    let mut output = Output::new("graphql", Some("123".into()));
    output.lists = Some(vec![fixture()]);
    output.complete = true;
    output.stop_reason = "single_list".into();
    output.pages = 1;
    let value = serde_json::to_value(&output).unwrap();
    assert_eq!(value["posts"], json!([]));
    assert!(value.get("users").is_none());
    assert_eq!(value["lists"][0]["visibility"], "private");
    let rendered = human(&value);
    for text in [
        "Synthetic list · 456",
        "Fixture description",
        "Visibility: private",
        "Owner: @owner_fixture · 789",
        "https://x.com/i/lists/456",
    ] {
        assert!(rendered.contains(text));
    }
    assert!(!rendered.contains("/status/"));
    output.lists.as_mut().unwrap()[0].visibility = Some(ListVisibility::Public);
    assert_eq!(
        serde_json::to_value(&output).unwrap()["lists"][0]["visibility"],
        "public"
    );
    let temp = tempfile::tempdir().unwrap();
    let cache = Cache::new(temp.path().canonicalize().unwrap().join("cache"));
    cache
        .put("graphql", Some("123"), "metadata", &output)
        .unwrap();
    let hit = cache
        .get("graphql", Some("123"), "metadata", 300)
        .unwrap()
        .unwrap();
    assert_eq!(hit.lists, output.lists);
    assert!(hit.posts.is_empty() && hit.users.is_none());
    assert_eq!(hit.provenance.cache, "hit");
    for account in [None, Some("999")] {
        assert!(
            cache
                .get("graphql", account, "metadata", 300)
                .unwrap()
                .is_none()
        );
    }
    assert!(
        cache
            .get("fxtwitter", None, "metadata", 300)
            .unwrap()
            .is_none()
    );
    cache.purge().unwrap();
    assert!(
        cache
            .get("graphql", Some("123"), "metadata", 300)
            .unwrap()
            .is_none()
    );
}
