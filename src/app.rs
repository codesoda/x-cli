mod auth;
mod retrieval;
mod task;
#[cfg(test)]
mod tests;

use crate::{
    cache::Cache,
    cli::{Access, CacheCommand, Cli, Command},
    config::Config,
    credentials::{Chrome, CredentialProvider, Profile},
    error::{Error, Kind, Result},
    model::Output,
    transport::Transport,
};
use serde_json::{Value, json};
use std::path::PathBuf;

pub struct SystemCredentials;
impl CredentialProvider for SystemCredentials {
    fn load(&self, profile: &str, consent: bool) -> Result<crate::credentials::Session> {
        Chrome::system()?.load(profile, consent)
    }
}
pub fn discover() -> Result<Vec<Profile>> {
    Chrome::system()?.discover()
}
pub fn data_root(cli: &Cli) -> Result<PathBuf> {
    cli.data_dir
        .clone()
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".xcli")))
        .ok_or_else(|| {
            Error::new(
                Kind::Storage,
                "Cannot locate home directory; specify --data-dir",
            )
        })
}
/// Dependency-injected entry point. Normal tests never need a real browser or network.
pub fn execute(
    cli: &Cli,
    t: &dyn Transport,
    credentials: &dyn CredentialProvider,
    discover: impl FnOnce() -> Result<Vec<Profile>>,
) -> Result<Value> {
    // Self-update never initializes accounts, caches, credentials or local
    // state, and it deliberately does not use the injected X transport.
    if let Command::Update { check, yes } = &cli.command {
        if cli.data_dir.is_some() {
            return Err(Error::new(
                Kind::InvalidInput,
                "update does not use --data-dir; remove the flag",
            ));
        }
        return crate::update::run(*check, *yes);
    }
    let root = data_root(cli)?;
    let cache = Cache::new(root.join("cache"));
    match &cli.command {
        Command::Doctor => {
            let config = Config::load(&root)?;
            Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"chrome_credentials_supported":cfg!(target_os="macos"),"connections":config.connections.len(),"data_directory":root,"checks":{"configuration":"valid","browser":"not inspected","keychain":"not accessed","network":"not contacted","authenticated_protocol":"source-verified, not live-verified"},"warnings":["doctor is local-only; auth add --consent performs live identity verification"]}),
            )
        }
        Command::Cache {
            command: CacheCommand::Purge,
        } => {
            cache.purge()?;
            Ok(json!({"purged":true,"rate_limit_cooldowns_preserved":true}))
        }
        Command::Auth { command } => {
            auth::execute(command, &root, &cache, t, credentials, discover)
        }
        command => {
            let (access, task) = task::Task::from_command(command)?;
            retrieval::execute(&root, &cache, access, task, t, credentials)
        }
    }
}
fn rate_call<T>(
    cache: &Cache,
    backend: &str,
    account: Option<&str>,
    request: impl FnOnce() -> Result<T>,
) -> Result<T> {
    cache.check_cooldown(backend, account)?;
    rate_result(cache, backend, account, request())
}
fn rate_result<T>(
    cache: &Cache,
    backend: &str,
    account: Option<&str>,
    result: Result<T>,
) -> Result<T> {
    if let Err(e) = &result
        && e.kind == Kind::RateLimit
    {
        cache.cooldown(backend, account, e.retry_after_seconds.unwrap_or(60))?;
    }
    result
}
fn cached(
    cache: &Cache,
    a: &Access,
    backend: &str,
    account: Option<&str>,
    key: &str,
) -> Result<Option<Output>> {
    if a.no_cache || a.refresh || a.cache_ttl == 0 {
        Ok(None)
    } else {
        cache.get(backend, account, key, a.cache_ttl)
    }
}
fn save_cached(cache: &Cache, a: &Access, key: &str, out: &Output) -> Result<()> {
    // Never cache a failed/truncated request as a successful fresh collection.
    if !a.no_cache
        && !out.request_failed
        && !matches!(
            out.stop_reason.as_str(),
            "request_failed" | "parent_unavailable" | "replies_failed"
        )
    {
        cache.put(
            &out.provenance.backend,
            out.provenance.account_id.as_deref(),
            key,
            out,
        )?;
    }
    Ok(())
}
pub fn human(value: &Value) -> String {
    if let Some(update) = crate::update::human(value) {
        return update;
    }
    if let Ok(out) = serde_json::from_value::<Output>(value.clone()) {
        let mut s = format!(
            "{} · {} · complete={} · {}\n",
            out.provenance.backend, out.provenance.cache, out.complete, out.stop_reason
        );
        for p in &out.posts {
            s.push_str(&format!(
                "\n@{} · {}\n{}\n{}\n",
                p.author.handle, p.id, p.text, p.url
            ));
        }
        if let Some(users) = &out.users {
            for user in users {
                s.push_str(&format!(
                    "\n@{} · {}\nhttps://x.com/{}\n",
                    user.handle, user.id, user.handle
                ));
            }
        }
        if let Some(cursor) = out.next_cursor {
            s.push_str(&format!("\nNext cursor: {cursor}\n"));
        }
        for warning in out.warnings {
            s.push_str(&format!("Warning: {warning}\n"));
        }
        s
    } else {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }
}
