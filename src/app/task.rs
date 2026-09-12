use crate::{
    cli::{
        Access, BookmarkCommand, Command, LikesCommand, ListMembersCommand, ListsCommand, Paging,
        RelationshipCommand, UserCommand,
    },
    error::{Error, Kind, Result},
};
use serde_json::json;

pub(super) enum Task {
    Read(String),
    Thread(String, u32, bool, Paging),
    Search(String, Paging),
    Timeline(String, Paging),
    Bookmarks(Paging),
    Likes(Paging),
    ListPosts(String, Paging),
    ListMembers(String, Paging),
    Relationships { followers: bool, paging: Paging },
}
impl Task {
    pub(super) fn from_command(command: &Command) -> Result<(&Access, Self)> {
        let (access, task) = match command {
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
            Command::Bookmarks {
                command: BookmarkCommand::List { access, paging },
            } => {
                if access
                    .account
                    .as_deref()
                    .is_none_or(|value| value.is_empty())
                {
                    return Err(Error::new(
                        Kind::InvalidInput,
                        "Private bookmark reads require an explicit --account selector",
                    ));
                }
                (access, Task::Bookmarks(paging.clone()))
            }
            Command::Likes {
                command: LikesCommand::List { access, paging },
            } => {
                if access
                    .account
                    .as_deref()
                    .is_none_or(|value| value.is_empty())
                {
                    return Err(Error::new(
                        Kind::InvalidInput,
                        "Own liked-post reads require an explicit --account selector",
                    ));
                }
                (access, Task::Likes(paging.clone()))
            }
            Command::Lists {
                command:
                    ListsCommand::Posts {
                        list_id,
                        access,
                        paging,
                    },
            }
            | Command::Lists {
                command:
                    ListsCommand::Members {
                        command:
                            ListMembersCommand::List {
                                list_id,
                                access,
                                paging,
                            },
                    },
            } => {
                if access
                    .account
                    .as_deref()
                    .is_none_or(|value| value.is_empty())
                {
                    return Err(Error::new(
                        Kind::InvalidInput,
                        "List reads require an explicit --account selector",
                    ));
                }
                let id = crate::input::id(list_id).map_err(|_| {
                    Error::new(
                        Kind::InvalidInput,
                        "Expected a positive decimal list ID (at most 20 digits)",
                    )
                })?;
                let task = if matches!(
                    command,
                    Command::Lists {
                        command: ListsCommand::Members { .. }
                    }
                ) {
                    Task::ListMembers(id, paging.clone())
                } else {
                    Task::ListPosts(id, paging.clone())
                };
                (access, task)
            }
            Command::Following {
                command: RelationshipCommand::List { access, paging },
            }
            | Command::Followers {
                command: RelationshipCommand::List { access, paging },
            } => {
                if access
                    .account
                    .as_deref()
                    .is_none_or(|value| value.is_empty())
                {
                    return Err(Error::new(
                        Kind::InvalidInput,
                        "Relationship reads require an explicit --account selector",
                    ));
                }
                (
                    access,
                    Task::Relationships {
                        followers: matches!(command, Command::Followers { .. }),
                        paging: paging.clone(),
                    },
                )
            }
            _ => unreachable!(),
        };
        task.validate()?;
        Ok((access, task))
    }
    pub(super) fn validate(&self) -> Result<()> {
        let paging = match self {
            Self::Read(_) => return Ok(()),
            Self::Thread(_, _, _, p)
            | Self::Search(_, p)
            | Self::Timeline(_, p)
            | Self::Bookmarks(p)
            | Self::Likes(p)
            | Self::ListPosts(_, p)
            | Self::ListMembers(_, p)
            | Self::Relationships { paging: p, .. } => p,
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
    pub(super) fn requires_graphql(&self) -> bool {
        matches!(
            self,
            Self::Search(..)
                | Self::Timeline(..)
                | Self::Bookmarks(..)
                | Self::Likes(..)
                | Self::ListPosts(..)
                | Self::ListMembers(..)
                | Self::Relationships { .. }
                | Self::Thread(_, _, true, _)
        )
    }
    pub(super) fn expects_users(&self) -> bool {
        matches!(self, Self::Relationships { .. } | Self::ListMembers(..))
    }
    pub(super) fn accepts_cached(&self, output: &crate::model::Output) -> bool {
        output.users.is_some() == self.expects_users()
            && (!self.expects_users() || output.posts.is_empty())
    }
    pub(super) fn key(&self) -> String {
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
            Self::Relationships {
                followers,
                paging: p,
            } => json!([
                "v1",
                "relationships",
                followers,
                p.max_pages,
                p.page_size,
                p.cursor
            ])
            .to_string(),
            Self::ListMembers(id, p) => {
                json!(["v1", "list_members", id, p.max_pages, p.page_size, p.cursor]).to_string()
            }
            Self::ListPosts(id, p) => {
                json!(["v1", "list_posts", id, p.max_pages, p.page_size, p.cursor]).to_string()
            }
            Self::Likes(p) => {
                json!(["v1", "own_likes", p.max_pages, p.page_size, p.cursor]).to_string()
            }
            Self::Bookmarks(p) => {
                json!(["v1", "bookmarks", p.max_pages, p.page_size, p.cursor]).to_string()
            }
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
