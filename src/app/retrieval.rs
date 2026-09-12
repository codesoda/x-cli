use super::{cached, rate_call, save_cached, task::Task};
use crate::{
    cache::Cache,
    cli::{Access, Backend, route},
    config::{Config, verify_identity},
    credentials::CredentialProvider,
    error::{Error, Kind, Result, storage},
    model::Output,
    pagination,
    providers::{fx, graphql::Graphql, operations::Operation},
    transport::Transport,
};
use serde_json::Value;
use std::path::Path;

pub(super) fn execute(
    root: &Path,
    cache: &Cache,
    access: &Access,
    task: Task,
    t: &dyn Transport,
    credentials: &dyn CredentialProvider,
) -> Result<Value> {
    let backend = route(access, task.requires_graphql())?;
    let key = task.key();
    let out = if backend == Backend::Fxtwitter {
        cache.check_cooldown("fxtwitter", None)?;
        if let Some(hit) = cached(cache, access, "fxtwitter", None, &key)? {
            hit
        } else {
            let fetch = |id: &str| rate_call(cache, "fxtwitter", None, || fx::read(t, id));
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
            save_cached(cache, access, &key, &out)?;
            out
        }
    } else {
        let config = Config::load(root)?;
        let connection = config.resolve(access.account.as_deref(), access.connection.as_deref())?;
        let account = Some(connection.identity.id.as_str());
        cache.check_cooldown("graphql", account)?;
        let session = credentials.load(&connection.profile, true)?;
        let graph = rate_call(cache, "graphql", account, || Graphql::new(t, &session))?;
        let actual = rate_call(cache, "graphql", account, || graph.identity())?;
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
        if let Some(hit) = cached(cache, access, "graphql", account, &key)?.filter(|hit| {
            // Old binaries can discard newly added fields while rewriting a scope.
            // Missing users/lists arrays are not successful empty typed results.
            task.accepts_cached(hit)
        }) {
            hit
        } else {
            let fetch =
                |id: &str| rate_call(cache, "graphql", account, || graph.read(id, &actual.id));
            let out = match &task {
                Task::Read(id) => fetch(id)?,
                Task::ListMetadata(id) => rate_call(cache, "graphql", account, || {
                    graph.list_metadata(id, &actual.id)
                })?,
                Task::Thread(id, max, replies, paging) => {
                    let mut chain = pagination::parents(fetch(id)?, *max, fetch)?;
                    if *replies {
                        let parent_complete = chain.complete;
                        let reply_result = pagination::collect(
                            Output::new("graphql", Some(actual.id.clone())),
                            paging.max_pages,
                            paging.cursor.clone(),
                            |cursor| {
                                rate_call(cache, "graphql", account, || {
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
                        rate_call(cache, "graphql", account, || {
                            graph.page(Operation::Search, query, paging.page_size, cursor)
                        })
                    },
                )?,
                Task::Relationships { followers, paging } => pagination::collect_users(
                    Output::new("graphql", Some(actual.id.clone())),
                    paging.max_pages,
                    paging.cursor.clone(),
                    |cursor| {
                        rate_call(cache, "graphql", account, || {
                            graph.users_page(
                                if *followers {
                                    Operation::Followers
                                } else {
                                    Operation::Following
                                },
                                &actual.id,
                                paging.page_size,
                                cursor,
                            )
                        })
                    },
                )?,
                Task::ListInventory(paging) => pagination::collect_lists(
                    Output::new("graphql", Some(actual.id.clone())),
                    paging.max_pages,
                    paging.cursor.clone(),
                    |cursor| {
                        rate_call(cache, "graphql", account, || {
                            graph.lists_page(&actual.id, paging.page_size, cursor)
                        })
                    },
                )?,
                Task::ListMembers(id, paging) => pagination::collect_users(
                    Output::new("graphql", Some(actual.id.clone())),
                    paging.max_pages,
                    paging.cursor.clone(),
                    |cursor| {
                        rate_call(cache, "graphql", account, || {
                            graph.users_page(Operation::ListMembers, id, paging.page_size, cursor)
                        })
                    },
                )?,
                Task::ListPosts(id, paging) => pagination::collect(
                    Output::new("graphql", Some(actual.id.clone())),
                    paging.max_pages,
                    paging.cursor.clone(),
                    |cursor| {
                        rate_call(cache, "graphql", account, || {
                            graph.page(Operation::ListPosts, id, paging.page_size, cursor)
                        })
                    },
                )?,
                Task::Likes(paging) => pagination::collect(
                    Output::new("graphql", Some(actual.id.clone())),
                    paging.max_pages,
                    paging.cursor.clone(),
                    |cursor| {
                        rate_call(cache, "graphql", account, || {
                            // Only the verified session owner's stable ID; no target selector.
                            graph.page(Operation::Likes, &actual.id, paging.page_size, cursor)
                        })
                    },
                )?,
                Task::Bookmarks(paging) => pagination::collect(
                    Output::new("graphql", Some(actual.id.clone())),
                    paging.max_pages,
                    paging.cursor.clone(),
                    |cursor| {
                        rate_call(cache, "graphql", account, || {
                            graph.page(Operation::Bookmarks, "", paging.page_size, cursor)
                        })
                    },
                )?,
                Task::Timeline(handle, paging) => {
                    let user = rate_call(cache, "graphql", account, || graph.user(handle))?;
                    pagination::collect(
                        Output::new("graphql", Some(actual.id.clone())),
                        paging.max_pages,
                        paging.cursor.clone(),
                        |cursor| {
                            rate_call(cache, "graphql", account, || {
                                graph.page(Operation::Timeline, &user.id, paging.page_size, cursor)
                            })
                        },
                    )?
                }
            };
            save_cached(cache, access, &key, &out)?;
            out
        }
    };
    serde_json::to_value(out).map_err(|_| storage())
}
