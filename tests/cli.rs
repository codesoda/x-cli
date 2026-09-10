use clap::Parser;
use std::{cell::Cell, process::Command};
use xcli::{
    cli::Cli,
    credentials::{CredentialProvider, Session},
    error::Result,
    transport::{Request, Response, Transport},
};
struct NoSecrets;
impl CredentialProvider for NoSecrets {
    fn load(&self, _: &str, _: bool) -> Result<Session> {
        panic!("normal test must not load credentials")
    }
}
struct PublicMock {
    calls: Cell<u32>,
}
impl Transport for PublicMock {
    fn get(&self, r: Request) -> Result<Response> {
        self.calls.set(self.calls.get() + 1);
        assert!(r.headers.is_empty());
        assert_eq!(r.url, "https://api.fxtwitter.com/2/status/20");
        Ok(Response{status:200,body:br#"{"code":200,"status":{"type":"status","id":"20","text":"synthetic public fixture","author":{"id":"12","screen_name":"fixture"},"replying_to":null}}"#.to_vec(),retry_after:None})
    }
}
#[test]
fn public_cache_roundtrip() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("state");
    let cli = Cli::try_parse_from([
        "xcli",
        "--data-dir",
        path.to_str().unwrap(),
        "read",
        "https://x.com/fixture/status/20",
    ])
    .unwrap();
    let t = PublicMock {
        calls: Cell::new(0),
    };
    let first = xcli::app::execute(&cli, &t, &NoSecrets, || panic!("no discovery")).unwrap();
    assert_eq!(first["posts"][0]["id"], "20");
    assert_eq!(first["provenance"]["cache"], "miss");
    let second = xcli::app::execute(&cli, &t, &NoSecrets, || panic!("no discovery")).unwrap();
    assert_eq!(second["provenance"]["cache"], "hit");
    assert_eq!(t.calls.get(), 1);
}
#[test]
fn incompatible_route_never_calls_transport() {
    let root = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "xcli",
        "--data-dir",
        root.path().canonicalize().unwrap().to_str().unwrap(),
        "read",
        "20",
        "--backend",
        "fxtwitter",
        "--account",
        "work",
    ])
    .unwrap();
    let t = PublicMock {
        calls: Cell::new(0),
    };
    let e = xcli::app::execute(&cli, &t, &NoSecrets, || panic!()).unwrap_err();
    assert_eq!(e.exit_code(), 7);
    assert_eq!(t.calls.get(), 0);
}
fn binary(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xcli"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn executable_help_and_safe_usage() {
    let out = binary(&["--help"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("read"));
    let out = binary(&["read", "20", "--auth-token", "SYNTHETIC_SECRET"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("SYNTHETIC_SECRET"));
    serde_json::from_slice::<serde_json::Value>(&out.stderr).unwrap();
}
#[test]
fn executable_doctor_no_browser() {
    let root = tempfile::tempdir().unwrap();
    let out = binary(&[
        "--data-dir",
        root.path().canonicalize().unwrap().to_str().unwrap(),
        "doctor",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["checks"]["keychain"], "not accessed");
}
#[test]
fn executable_requires_consent() {
    let root = tempfile::tempdir().unwrap();
    let out = binary(&[
        "--data-dir",
        root.path().canonicalize().unwrap().to_str().unwrap(),
        "auth",
        "add",
        "--profile",
        "Default",
    ]);
    assert_eq!(out.status.code(), Some(5));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&out.stderr).unwrap()["kind"],
        "consent_required"
    );
}
#[test]
fn executable_invalid_url() {
    let out = binary(&["read", "https://evil.example/user/status/20"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}

/// Explicit opt-in only: `cargo test --test cli live_public -- --ignored`.
#[test]
#[ignore = "contacts FxTwitter; no credentials"]
fn live_public() {
    let root = tempfile::tempdir().unwrap();
    let out = binary(&[
        "--data-dir",
        root.path().canonicalize().unwrap().to_str().unwrap(),
        "read",
        "20",
        "--no-cache",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["posts"][0]["id"], "20");
}
