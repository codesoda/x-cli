use crate::{
    cache::Cache,
    cli::{Access, AuthCommand, Backend, CacheCommand, Cli, Command, Paging, UserCommand, route},
    config::{Config, verify_identity},
    credentials::{Chrome, CredentialProvider, Profile},
    error::{Error, Kind, Result, storage},
    model::{Output, now},
    pagination,
    providers::{fx, graphql::Graphql, operations::Operation},
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
    let root = data_root(cli)?;
    let cache = Cache::new(root.join("cache"));
    match &cli.command {
        Command::Doctor => {
            let config = Config::load(&root)?;
            return Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"chrome_credentials_supported":cfg!(target_os="macos"),"connections":config.connections.len(),"data_directory":root,"checks":{"configuration":"valid","browser":"not inspected","keychain":"not accessed","network":"not contacted","authenticated_protocol":"source-verified, not live-verified"},"warnings":["doctor is local-only; auth add --consent performs live identity verification"]}),
            );
        }
        Command::Cache {
            command: CacheCommand::Purge,
        } => {
            cache.purge()?;
            return Ok(json!({"purged":true,"rate_limit_cooldowns_preserved":true}));
        }
        Command::Auth { command } => {
            if matches!(command, AuthCommand::Discover) {
                return serde_json::to_value(discover()?).map_err(|_| storage());
            }
            let config = Config::load(&root)?;
            match command {
                AuthCommand::List => return serde_json::to_value(config).map_err(|_| storage()),
                AuthCommand::Add {
                    profile,
                    alias,
                    consent,
                    ..
                } => {
                    if !consent {
                        return Err(Error::new(
                            Kind::ConsentRequired,
                            "Connecting requires --consent for local X cookie/Keychain access and a read-only X identity request",
                        ));
                    }
                    // Validate connection syntax and uniqueness before any credential access.
                    let mut trial = config.clone();
                    trial.add(
                        profile.clone(),
                        alias.clone(),
                        crate::model::Identity {
                            id: "1".into(),
                            handle: "validation".into(),
                        },
                    )?;
                    cache.check_cooldown("graphql", None)?;
                    let result = (|| {
                        let session = credentials.load(profile, true)?;
                        let graph = Graphql::new(t, &session)?;
                        graph.identity()
                    })();
                    let identity = rate_result(&cache, "graphql", None, result)?;
                    let connection = Config::update(&root, |current| {
                        current.add(profile.clone(), alias.clone(), identity)
                    })?;
                    return Ok(
                        json!({"connection":connection,"verified_at":now(),"warnings":["Connection permits on-demand session loading for read-only requests. Browser cookies are account-level credentials."]}),
                    );
                }
                AuthCommand::Discover => unreachable!(),
                _ => {}
            }
            return Config::update(&root, |current| {
                match command {
                    AuthCommand::Remove { connection } => current.remove(connection)?,
                    AuthCommand::Default { account } => current.set_default(account)?,
                    AuthCommand::Prefer { connection } => current.prefer(connection)?,
                    AuthCommand::Rename { connection, alias } => {
                        current.rename(connection, alias.clone())?
                    }
                    _ => unreachable!(),
                }
                serde_json::to_value(current).map_err(|_| storage())
            });
        }
        _ => {}
    }
    let (access, task) = match &cli.command {
        Command::Read { target, access } => (access, Task::Read(crate::input::post(target)?)),
        Command::Thread {
            target,
            access,
            max_parents,
            replies,
            paging,
        } => {
            if !replies && (paging.cursor.is_some() || paging.max_pages.is_some()) {
                return Err(Error::new(
                    Kind::Unsupported,
                    "Reply pagination flags require --replies",
                ));
            }
            (
                access,
                Task::Thread(
                    crate::input::post(target)?,
                    *max_parents,
                    *replies,
                    Paging {
                        max_pages: paging.max_pages.unwrap_or(1),
                        page_size: 20,
                        cursor: paging.cursor.clone(),
                    },
                ),
            )
        }
        Command::Search {
            query,
            access,
            paging,
        } => {
            if query.trim().is_empty() || query.len() > 4096 {
                return Err(Error::new(
                    Kind::InvalidInput,
                    "Search query must be nonempty and at most 4096 bytes",
                ));
            }
            (access, Task::Search(query.clone(), paging.clone()))
        }
        Command::User {
            command:
                UserCommand::Posts {
                    handle,
                    access,
                    paging,
                },
        } => (
            access,
            Task::Timeline(crate::input::handle(handle)?, paging.clone()),
        ),
        _ => unreachable!(),
    };
    task.validate()?;
    let backend = route(access, task.requires_graphql())?;
    let key = task.key();
    let out = if backend == Backend::Fxtwitter {
        cache.check_cooldown("fxtwitter", None)?;
        if let Some(hit) = cached(&cache, access, "fxtwitter", None, &key)? {
            hit
        } else {
            let fetch = |id: &str| rate_call(&cache, "fxtwitter", None, || fx::read(t, id));
            let out = match &task {
                Task::Read(id) => fetch(id)?,
                Task::Thread(id, max, _, _) => pagination::parents(fetch(id)?, *max, fetch)?,
                _ => {
                    return Err(Error::new(
                        Kind::Unsupported,
                        "Operation requires authenticated GraphQL",
                    ));
                }
            };
            save_cached(&cache, access, &key, &out)?;
            out
        }
    } else {
        let config = Config::load(&root)?;
        let connection = config.resolve(access.account.as_deref(), access.connection.as_deref())?;
        let account = Some(connection.identity.id.as_str());
        cache.check_cooldown("graphql", account)?;
        let session = credentials.load(&connection.profile, true)?;
        let graph = rate_call(&cache, "graphql", account, || Graphql::new(t, &session))?;
        let actual = rate_call(&cache, "graphql", account, || graph.identity())?;
        verify_identity(&connection.identity, &actual)?;
        if access
            .account
            .as_deref()
            .and_then(|s| s.strip_prefix('@'))
            .is_some_and(|h| !actual.handle.eq_ignore_ascii_case(h))
        {
            return Err(Error::new(
                Kind::InvalidInput,
                "Account handle changed; use its alias/connection or explicitly reconnect",
            ));
        }
        // Identity verification always precedes authenticated cache access.
        if let Some(hit) = cached(&cache, access, "graphql", account, &key)? {
            hit
        } else {
            let fetch =
                |id: &str| rate_call(&cache, "graphql", account, || graph.read(id, &actual.id));
            let out = match &task {
                Task::Read(id) => fetch(id)?,
                Task::Thread(id, max, replies, paging) => {
                    let mut chain = pagination::parents(fetch(id)?, *max, fetch)?;
                    if *replies {
                        let parent_complete = chain.complete;
                        let reply_result = pagination::collect(
                            Output::new("graphql", Some(actual.id.clone())),
                            paging.max_pages,
                            paging.cursor.clone(),
                            |cursor| {
                                rate_call(&cache, "graphql", account, || {
                                    graph.page(Operation::Detail, id, paging.page_size, cursor)
                                })
                            },
                        );
                        match reply_result {
                            Ok(mut replies) => {
                                let known: std::collections::HashSet<_> =
                                    chain.posts.iter().map(|p| p.id.clone()).collect();
                                replies.posts.retain(|p| !known.contains(&p.id));
                                chain.request_failed |= replies.request_failed;
                                chain.posts.extend(replies.posts);
                                chain.pages += replies.pages;
                                chain.next_cursor = replies.next_cursor;
                                chain.stop_reason = replies.stop_reason;
                                chain.warnings.extend(replies.warnings);
                            }
                            Err(e) => {
                                chain.request_failed = true;
                                chain.stop_reason = "replies_failed".into();
                                chain.warnings.push(format!(
                                    "Reply retrieval failed: {} (exit code {})",
                                    e.message,
                                    e.exit_code()
                                ));
                            }
                        }
                        chain.complete = false;
                        chain.replies_complete = Some(false);
                        chain.warnings.push(format!("Parent chain complete: {parent_complete}. Replies are an incomplete upstream conversation view, which may include ancestors or other branches."));
                    }
                    chain
                }
                Task::Search(query, paging) => pagination::collect(
                    Output::new("graphql", Some(actual.id.clone())),
                    paging.max_pages,
                    paging.cursor.clone(),
                    |cursor| {
                        rate_call(&cache, "graphql", account, || {
                            graph.page(Operation::Search, query, paging.page_size, cursor)
                        })
                    },
                )?,
                Task::Timeline(handle, paging) => {
                    let user = rate_call(&cache, "graphql", account, || graph.user(handle))?;
                    pagination::collect(
                        Output::new("graphql", Some(actual.id.clone())),
                        paging.max_pages,
                        paging.cursor.clone(),
                        |cursor| {
                            rate_call(&cache, "graphql", account, || {
                                graph.page(Operation::Timeline, &user.id, paging.page_size, cursor)
                            })
                        },
                    )?
                }
            };
            save_cached(&cache, access, &key, &out)?;
            out
        }
    };
    serde_json::to_value(out).map_err(|_| storage())
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
enum Task {
    Read(String),
    Thread(String, u32, bool, Paging),
    Search(String, Paging),
    Timeline(String, Paging),
}
impl Task {
    fn validate(&self) -> Result<()> {
        let paging = match self {
            Self::Read(_) => return Ok(()),
            Self::Thread(_, _, _, p) | Self::Search(_, p) | Self::Timeline(_, p) => p,
        };
        if paging
            .cursor
            .as_ref()
            .is_some_and(|c| c.is_empty() || c.len() > 4096)
        {
            return Err(Error::new(
                Kind::InvalidInput,
                "Cursor must contain 1 to 4096 bytes",
            ));
        }
        Ok(())
    }
    fn requires_graphql(&self) -> bool {
        matches!(
            self,
            Self::Search(..) | Self::Timeline(..) | Self::Thread(_, _, true, _)
        )
    }
    fn key(&self) -> String {
        match self {
            Self::Read(id) => json!(["v1", "read", id]).to_string(),
            Self::Thread(id, max, replies, p) => json!([
                "v1",
                "thread",
                id,
                max,
                replies,
                p.max_pages,
                p.page_size,
                p.cursor
            ])
            .to_string(),
            Self::Search(q, p) => {
                json!(["v1", "search", q, p.max_pages, p.page_size, p.cursor]).to_string()
            }
            Self::Timeline(h, p) => json!([
                "v1",
                "timeline",
                h.to_lowercase(),
                p.max_pages,
                p.page_size,
                p.cursor
            ])
            .to_string(),
        }
    }
}
pub fn human(value: &Value) -> String {
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
