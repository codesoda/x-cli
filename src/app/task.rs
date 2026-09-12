use crate::{
    cli::{Access, BookmarkCommand, Command, Paging, UserCommand},
    error::{Error, Kind, Result},
};
use serde_json::json;

pub(super) enum Task {
    Read(String),
    Thread(String, u32, bool, Paging),
    Search(String, Paging),
    Timeline(String, Paging),
    Bookmarks(Paging),
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
            | Self::Bookmarks(p) => p,
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
                | Self::Thread(_, _, true, _)
        )
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
