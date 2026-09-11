//! Offline executable-level update contract tests. These never resolve or
//! download a release: every invocation fails argument validation or the
//! prompt requirement before any network access would occur.
use std::process::{Command, Stdio};

fn binary(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xcli"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn stderr_json(out: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&out.stderr).expect("stderr must be machine-readable JSON")
}

#[test]
fn update_check_conflicts_with_yes() {
    let out = binary(&["update", "--check", "--yes"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_eq!(stderr_json(&out)["kind"], "invalid_input");
}

#[test]
fn non_tty_update_without_yes_fails_promptly() {
    // stdin is explicitly not a terminal; the error must arrive without any
    // network access and point at --yes.
    let out = binary(&["update"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let error = stderr_json(&out);
    assert_eq!(error["kind"], "invalid_input");
    assert!(
        error["message"].as_str().unwrap().contains("--yes"),
        "{error}"
    );
}

#[test]
fn update_rejects_unrelated_flags_instead_of_ignoring_them() {
    for args in [
        ["update", "--account", "work"].as_slice(),
        &["update", "--backend", "graphql"],
        &["update", "--max-pages", "2"],
        &["update", "--cursor", "abc"],
        &["update", "--no-cache"],
    ] {
        let out = binary(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(stderr_json(&out)["kind"], "invalid_input");
    }
}

#[test]
fn update_rejects_data_dir_rather_than_ignoring_it() {
    let root = tempfile::tempdir().unwrap();
    let out = binary(&[
        "--data-dir",
        root.path().to_str().unwrap(),
        "update",
        "--check",
    ]);
    assert_eq!(out.status.code(), Some(2));
    let error = stderr_json(&out);
    assert_eq!(error["kind"], "invalid_input");
    assert!(
        error["message"].as_str().unwrap().contains("--data-dir"),
        "{error}"
    );
    // The unused state directory stays untouched.
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
