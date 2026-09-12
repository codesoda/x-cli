//! Local opt-in smoke tests. Never print captured payloads, session material or headers.
//! See docs/live-verification.md; enabling the feature alone performs no live work.
#[path = "live/list_inventory.rs"]
mod list_inventory;
#[path = "live/list_metadata.rs"]
mod list_metadata;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use xcli::{
    config::Config, credentials::Session, model::Output, providers::graphql::Graphql,
    transport::Http,
};

fn permit(
    ci: bool,
    live: Option<&str>,
    auth: Option<&str>,
    needs_auth: bool,
) -> Result<(), &'static str> {
    if ci {
        return Err("Live tests refuse CI execution");
    }
    if live != Some("1") {
        return Err("Set XCLI_LIVE=1 to opt into local network tests");
    }
    if needs_auth && auth != Some("1") {
        return Err(
            "Set XCLI_LIVE_AUTH=1 to consent to local browser-session loading and authenticated reads",
        );
    }
    Ok(())
}
fn require_opt_in(auth: bool) {
    let ci = [
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "TF_BUILD",
        "JENKINS_URL",
        "BUILDKITE",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some());
    let live = std::env::var("XCLI_LIVE").ok();
    let consent = std::env::var("XCLI_LIVE_AUTH").ok();
    permit(ci, live.as_deref(), consent.as_deref(), auth)
        .unwrap_or_else(|message| panic!("{message}"));
}
fn root() -> PathBuf {
    std::env::var_os("XCLI_LIVE_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".xcli")))
        .expect("Set XCLI_LIVE_DATA_DIR when HOME is unavailable")
}
struct PublicAssetTransport(Http);
impl xcli::transport::Transport for PublicAssetTransport {
    fn get(
        &self,
        request: xcli::transport::Request,
    ) -> xcli::error::Result<xcli::transport::Response> {
        if !request.headers.is_empty() || request.url != xcli::providers::operations::BUNDLE_URL {
            return Err(xcli::error::Error::new(
                xcli::error::Kind::Unsupported,
                "Public asset test refused a credential-bearing or unexpected request",
            ));
        }
        self.0.get(request)
    }
}
fn required(name: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            panic!("Set {name} explicitly; no account/profile is selected automatically")
        })
}
fn safe_diagnostic(stderr: &[u8]) -> Option<xcli::error::Diagnostic> {
    if stderr.len() > 64 * 1024 {
        return None;
    }
    let error: serde_json::Value = serde_json::from_slice(stderr).ok()?;
    serde_json::from_value(error.get("diagnostic")?.clone()).ok()
}
fn select_binary(override_path: Option<&std::ffi::OsStr>) -> Result<PathBuf, &'static str> {
    let path = override_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_xcli")));
    if !path.is_absolute() || !path.is_file() {
        return Err("XCLI_LIVE_BINARY must name an existing absolute executable file");
    }
    Ok(path)
}

fn run(root: &Path, args: &[&str], stage: &str) -> Output {
    let binary = select_binary(std::env::var_os("XCLI_LIVE_BINARY").as_deref())
        .unwrap_or_else(|message| panic!("{message}"));
    let result = Command::new(binary)
        .arg("--data-dir")
        .arg(root)
        .args(args)
        .arg("--no-cache")
        .output()
        .unwrap_or_else(|_| panic!("{stage}: cannot launch xcli"));
    assert!(
        matches!(result.status.code(), Some(0 | 12)),
        "{stage}: xcli failed with exit {:?}; diagnostic={:?}; payload and raw stderr withheld",
        result.status.code(),
        safe_diagnostic(&result.stderr)
    );
    let output: Output = serde_json::from_slice(&result.stdout)
        .unwrap_or_else(|_| panic!("{stage}: invalid normalized output; payload withheld"));
    assert!(
        !output.request_failed,
        "{stage}: a request failed within the partial result; payload withheld"
    );
    assert!(
        output.provenance.cache == "miss",
        "{stage}: expected an upstream read"
    );
    assert!(
        output
            .next_cursor
            .as_ref()
            .is_none_or(|cursor| !cursor.is_empty())
    );
    output
}

#[test]
fn binary_selection_is_explicit_and_fail_closed() {
    use std::ffi::OsStr;
    assert_eq!(
        select_binary(None).unwrap(),
        PathBuf::from(env!("CARGO_BIN_EXE_xcli"))
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("installed-xcli");
    std::fs::write(&binary, b"synthetic executable placeholder").unwrap();
    assert_eq!(select_binary(Some(binary.as_os_str())).unwrap(), binary);
    for path in [
        OsStr::new(""),
        OsStr::new("relative-xcli"),
        directory.path().as_os_str(),
    ] {
        assert!(select_binary(Some(path)).is_err());
    }
    assert!(select_binary(Some(directory.path().join("missing").as_os_str())).is_err());
}

#[test]
fn diagnostics_never_forward_upstream_strings() {
    use xcli::error::Diagnostic;
    assert_eq!(
        safe_diagnostic(
            br#"{"message":"SYNTHETIC_SECRET","diagnostic":{"stage":"http","status":400}}"#
        ),
        Some(Diagnostic::Http { status: 400 })
    );
    for stderr in [
        br#"{"diagnostic":{"stage":"SYNTHETIC_SECRET"}}"#.as_slice(),
        br#"{"diagnostic":{"stage":"http","status":400,"message":"SYNTHETIC_SECRET"}}"#.as_slice(),
        b"SYNTHETIC_SECRET".as_slice(),
    ] {
        assert!(safe_diagnostic(stderr).is_none());
    }
}
#[test]
fn live_guards_are_fail_closed() {
    for auth in [false, true] {
        assert!(permit(true, Some("1"), Some("1"), auth).is_err());
        assert!(permit(false, None, Some("1"), auth).is_err());
    }
    assert!(permit(false, Some("1"), None, true).is_err());
    assert!(permit(false, Some("1"), Some("0"), true).is_err());
    assert!(permit(false, Some("1"), None, false).is_ok());
    assert!(permit(false, Some("1"), Some("1"), true).is_ok());
}

#[test]
#[ignore = "local public network test; requires XCLI_LIVE=1"]
fn public_post_and_parent_chain() {
    require_opt_in(false);
    let root = root();
    let post = run(
        &root,
        &["read", "20", "--backend", "fxtwitter"],
        "public post",
    );
    assert!(post.complete && post.posts.len() == 1 && post.posts[0].id == "20");
    assert!(post.provenance.backend == "fxtwitter" && post.provenance.account_id.is_none());
    let chain = run(
        &root,
        &[
            "thread",
            "20",
            "--backend",
            "fxtwitter",
            "--max-parents",
            "1",
        ],
        "public parents",
    );
    assert!(chain.parent_chain_complete == Some(true) && chain.complete);
    assert!(chain.posts.len() == 1 && chain.posts[0].id == "20");
}

#[test]
#[ignore = "local public asset test; requires XCLI_LIVE=1; no authenticated query"]
fn public_manifest() {
    require_opt_in(false);
    let transport = PublicAssetTransport(Http::new().expect("Cannot construct HTTPS transport"));
    // Synthetic session is never used: construction fetches only the credential-free asset.
    let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
    assert!(
        Graphql::new(&transport, &session).is_ok(),
        "Pinned public manifest unavailable or changed"
    );
}

fn authenticated_connection() -> (PathBuf, String, xcli::config::Connection) {
    require_opt_in(true);
    // Capability check only; constructing Chrome does not inspect its profile or Keychain.
    xcli::credentials::Chrome::system()
        .expect("Authenticated smoke test requires macOS Chrome support");
    let account = required("XCLI_LIVE_ACCOUNT");
    let profile = required("XCLI_LIVE_PROFILE");
    let root = root();
    let config = Config::load(&root).expect("Cannot load local account configuration");
    let explicit_connection = std::env::var("XCLI_LIVE_CONNECTION").ok();
    let connection = config.resolve(Some(&account), explicit_connection.as_deref())
        .expect("Connect the requested account first; select a preference or explicit connection if ambiguous");
    assert!(
        connection.profile == profile,
        "Selected connection does not match XCLI_LIVE_PROFILE; no credentials loaded"
    );
    (root, account, connection.clone())
}

#[test]
#[ignore = "local private bookmark test; requires consent and explicit registered account/profile"]
fn authenticated_bookmark_smoke() {
    private_collection_smoke(&["bookmarks", "list"], "authenticated bookmarks");
}

#[test]
#[ignore = "local private likes test; requires consent and explicit registered account/profile"]
fn authenticated_own_likes_smoke() {
    private_collection_smoke(&["likes", "list"], "authenticated own likes");
}

#[test]
#[ignore = "local list-post test; requires consent, explicit account/profile and list ID"]
fn authenticated_list_posts_smoke() {
    require_opt_in(true);
    let list_id = required("XCLI_LIVE_LIST_ID");
    xcli::input::id(&list_id).expect("XCLI_LIVE_LIST_ID must be a positive decimal list ID");
    private_collection_smoke(&["lists", "posts", &list_id], "authenticated list posts");
}

#[test]
#[ignore = "local following read; requires consent and explicit account/profile"]
fn authenticated_following_smoke() {
    private_collection_smoke(&["following", "list"], "authenticated following");
}
#[test]
#[ignore = "local followers read; requires consent and explicit account/profile"]
fn authenticated_followers_smoke() {
    private_collection_smoke(&["followers", "list"], "authenticated followers");
}

#[test]
#[ignore = "local list-member read; requires consent, account/profile and list ID"]
fn authenticated_list_members_smoke() {
    require_opt_in(true);
    let list_id = required("XCLI_LIVE_LIST_ID");
    xcli::input::id(&list_id).expect("XCLI_LIVE_LIST_ID must be a positive decimal list ID");
    private_collection_smoke(
        &["lists", "members", "list", &list_id],
        "authenticated list members",
    );
}

fn private_collection_smoke(operation: &[&str], stage: &str) {
    let (root, account, connection) = authenticated_connection();
    let mut args = operation.to_vec();
    args.extend([
        "--account",
        &account,
        "--connection",
        &connection.id,
        "--max-pages",
        "1",
        "--page-size",
        "5",
    ]);
    let output = run(&root, &args, stage);
    assert!(output.provenance.backend == "graphql");
    assert!(
        output.provenance.account_id.as_ref() == Some(&connection.identity.id),
        "Account provenance mismatch; values withheld"
    );
    assert!(
        output.pages == 1 && !output.complete,
        "Private collection must be bounded and conservatively incomplete"
    );
    if matches!(operation[0], "following" | "followers")
        || operation.starts_with(&["lists", "members"])
    {
        assert!(
            output.users.is_some() && output.posts.is_empty(),
            "Expected a user collection"
        );
    }
    // Empty collections are valid. No private content/IDs are ever printed.
}

#[test]
#[ignore = "local authenticated test; requires consent and explicit registered account/profile"]
fn authenticated_read_smoke() {
    let (root, account, connection) = authenticated_connection();
    let expected_id = &connection.identity.id;
    // One sequential test: stop immediately on failure/rate limit, never rotate accounts.
    // Existing state is retained so cooldowns survive test failures and subsequent runs.
    for (stage, operation) in [
        ("authenticated post", vec!["read", "20"]),
        (
            "authenticated parents",
            vec!["thread", "20", "--max-parents", "1"],
        ),
        (
            "authenticated replies",
            vec![
                "thread",
                "20",
                "--replies",
                "--max-parents",
                "1",
                "--max-pages",
                "2",
            ],
        ),
        (
            "authenticated search",
            vec![
                "search",
                "from:jack",
                "--max-pages",
                "2",
                "--page-size",
                "5",
            ],
        ),
        (
            "authenticated timeline",
            vec![
                "user",
                "posts",
                "jack",
                "--max-pages",
                "2",
                "--page-size",
                "5",
            ],
        ),
    ] {
        let mut args = operation;
        args.extend(["--account", &account, "--connection", &connection.id]);
        let output = run(&root, &args, stage);
        assert!(output.provenance.backend == "graphql");
        assert!(
            output.provenance.account_id.as_ref() == Some(expected_id),
            "Account provenance mismatch; values withheld"
        );
        let max_pages = match stage {
            "authenticated replies" => 3,
            "authenticated search" | "authenticated timeline" => 2,
            _ => 1,
        };
        assert!(
            output.pages > 0 && output.pages <= max_pages,
            "Unexpected pagination bound"
        );
        if matches!(stage, "authenticated post" | "authenticated parents") {
            assert!(output.complete && output.posts.len() == 1 && output.posts[0].id == "20");
        } else {
            assert!(
                !output.complete,
                "Collection must not claim exhaustive completeness"
            );
        }
    }
}
