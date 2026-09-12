use crate::error::{Error, Kind, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "xcli",
    version,
    about = "Read-only X/Twitter CLI. JSON by default; no mutations."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    #[arg(long, global = true, help = "Local state directory (default ~/.xcli)")]
    pub data_dir: Option<PathBuf>,
    #[arg(long, global = true, help = "Human-readable output instead of JSON")]
    pub human: bool,
}
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Read a public post, or select an account for authenticated access.
    Read {
        target: String,
        #[command(flatten)]
        access: Access,
    },
    /// Walk parents and optionally fetch authenticated replies; never claims a full reply tree.
    Thread {
        target: String,
        #[command(flatten)]
        access: Access,
        #[arg(long,default_value_t=20,value_parser=clap::value_parser!(u32).range(1..=100))]
        max_parents: u32,
        #[arg(long, help = "Retrieve replies (requires GraphQL)")]
        replies: bool,
        #[command(flatten)]
        paging: ReplyPaging,
    },
    /// Search X (requires an explicitly connected account).
    Search {
        query: String,
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Read private bookmarks (requires explicit --account; no bookmark mutations).
    Bookmarks {
        #[command(subcommand)]
        command: BookmarkCommand,
    },
    /// Read the selected account's own liked posts (no like/unlike mutations).
    Likes {
        #[command(subcommand)]
        command: LikesCommand,
    },
    /// Read viewer-visible list inventory or a known list (no mutations).
    Lists {
        #[command(subcommand)]
        command: ListsCommand,
    },
    /// List accounts followed by the selected account (no follow/unfollow writes).
    Following {
        #[command(subcommand)]
        command: RelationshipCommand,
    },
    /// List followers of the selected account (not an exhaustive social graph).
    Followers {
        #[command(subcommand)]
        command: RelationshipCommand,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
    /// Local diagnostics only: does not inspect profiles, access Keychain, or contact X.
    Doctor,
    /// Explicitly check for or install a newer released xcli; never runs automatically.
    Update {
        #[arg(long, help = "Report update status without changing the installation")]
        check: bool,
        #[arg(
            short = 'y',
            long,
            conflicts_with = "check",
            help = "Skip the interactive install confirmation"
        )]
        yes: bool,
    },
}
#[derive(Debug, Subcommand)]
pub enum UserCommand {
    Posts {
        handle: String,
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
}
#[derive(Debug, Subcommand)]
pub enum BookmarkCommand {
    /// List the selected account's bookmarks; never claims an exhaustive collection.
    List {
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
}
#[derive(Debug, Subcommand)]
pub enum LikesCommand {
    /// List your own liked posts; requires explicit --account, never a target user.
    List {
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
}
#[derive(Debug, Subcommand)]
pub enum ListsCommand {
    /// Read the viewer-visible management inventory; explicit --account, not exhaustive.
    List {
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
    /// Read a single list's metadata; requires its decimal ID and explicit --account.
    Show {
        list_id: String,
        #[command(flatten)]
        access: Access,
    },
    /// Read list membership; no membership changes.
    Members {
        #[command(subcommand)]
        command: ListMembersCommand,
    },
    /// Read the latest-post view of a list; requires its decimal ID and explicit --account.
    Posts {
        list_id: String,
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
}
#[derive(Debug, Subcommand)]
pub enum ListMembersCommand {
    /// Read the users in a known list; requires explicit --account.
    List {
        list_id: String,
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
}
#[derive(Debug, Subcommand)]
pub enum RelationshipCommand {
    /// Read the verified account's own relationship view; requires explicit --account.
    List {
        #[command(flatten)]
        access: Access,
        #[command(flatten)]
        paging: Paging,
    },
}
#[derive(Debug, Subcommand)]
pub enum CacheCommand {
    /// Purge all public and authenticated cached content, preserving rate-limit cooldowns.
    Purge,
}
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Enumerate Chrome profile directories; does not read cookies or Keychain.
    Discover,
    /// Connect a profile, verifying and pinning the live X identity.
    Add {
        #[arg(long,default_value="chrome",value_parser=["chrome"])]
        browser: String,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        alias: Option<String>,
        #[arg(
            long,
            help = "Consent to local X cookie/Keychain access and read-only identity request"
        )]
        consent: bool,
    },
    List,
    /// Remove a local connection registration (does not log out Chrome).
    Remove {
        connection: String,
    },
    Default {
        account: String,
    },
    /// Prefer a connection when several profiles authenticate the same X account.
    Prefer {
        connection: String,
    },
    /// Rename a local connection alias; omit --alias to clear it.
    Rename {
        connection: String,
        #[arg(long)]
        alias: Option<String>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    Auto,
    Fxtwitter,
    Graphql,
}
#[derive(Debug, Clone, clap::Args)]
pub struct Access {
    #[arg(
        long,
        help = "@handle or local alias (implies GraphQL unless backend is specified)"
    )]
    pub account: Option<String>,
    #[arg(
        long,
        help = "Explicit local connection ID when an account has multiple profiles"
    )]
    pub connection: Option<String>,
    #[arg(long,value_enum,default_value_t=Backend::Auto)]
    pub backend: Backend,
    #[arg(long, help = "Bypass cache reads and writes")]
    pub no_cache: bool,
    #[arg(
        long,
        conflicts_with = "no_cache",
        help = "Refresh from upstream and replace cached result"
    )]
    pub refresh: bool,
    #[arg(long,default_value_t=300,value_parser=clap::value_parser!(u64).range(0..=86400),help="Maximum cached response age in seconds")]
    pub cache_ttl: u64,
}
#[derive(Debug, Clone, clap::Args)]
pub struct ReplyPaging {
    #[arg(long,requires="replies",value_parser=clap::value_parser!(u32).range(1..=20))]
    pub max_pages: Option<u32>,
    #[arg(long, requires = "replies")]
    pub cursor: Option<String>,
}
#[derive(Debug, Clone, clap::Args)]
pub struct Paging {
    #[arg(long,default_value_t=1,value_parser=clap::value_parser!(u32).range(1..=20))]
    pub max_pages: u32,
    #[arg(long,default_value_t=20,value_parser=clap::value_parser!(u32).range(1..=100),help="Requested page size; upstream may return fewer or more")]
    pub page_size: u32,
    #[arg(long, help = "Opaque continuation cursor from a previous response")]
    pub cursor: Option<String>,
}
pub fn route(access: &Access, requires_graphql: bool) -> Result<Backend> {
    let has_account = access.account.is_some() || access.connection.is_some();
    match access.backend {
        Backend::Fxtwitter if has_account || requires_graphql => Err(Error::new(
            Kind::Unsupported,
            "FxTwitter cannot serve account-selected or authenticated-only operations",
        )),
        Backend::Auto if has_account || requires_graphql => Ok(Backend::Graphql),
        Backend::Auto => Ok(Backend::Fxtwitter),
        b => Ok(b),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn access(b: Backend, account: Option<&str>) -> Access {
        Access {
            account: account.map(str::to_owned),
            connection: None,
            backend: b,
            no_cache: false,
            refresh: false,
            cache_ttl: 300,
        }
    }
    #[test]
    fn routing() {
        assert_eq!(
            route(&access(Backend::Auto, None), false).unwrap(),
            Backend::Fxtwitter
        );
        assert_eq!(
            route(&access(Backend::Auto, Some("work")), false).unwrap(),
            Backend::Graphql
        );
        assert_eq!(
            route(&access(Backend::Auto, None), true).unwrap(),
            Backend::Graphql
        );
        assert!(route(&access(Backend::Fxtwitter, Some("work")), false).is_err());
        assert!(route(&access(Backend::Fxtwitter, None), true).is_err());
    }
    #[test]
    fn cli_contract() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
        assert!(Cli::try_parse_from(["xcli", "read", "20", "--alias", "work"]).is_err());
        assert!(Cli::try_parse_from(["xcli", "search", "q", "--max-pages", "0"]).is_err());
        assert!(Cli::try_parse_from(["xcli", "likes", "add", "20"]).is_err());
    }
    #[test]
    fn bookmarks_contract() {
        let cli = Cli::try_parse_from([
            "xcli",
            "bookmarks",
            "list",
            "--account",
            "work",
            "--max-pages",
            "2",
            "--page-size",
            "5",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Bookmarks {
                command: BookmarkCommand::List { .. }
            }
        ));
        for args in [
            vec!["xcli", "bookmarks", "add", "20", "--account", "work"],
            vec!["xcli", "bookmarks", "remove", "20", "--account", "work"],
            vec!["xcli", "bookmarks", "list", "--max-pages", "0"],
            vec!["xcli", "bookmarks", "list", "--page-size", "101"],
            vec!["xcli", "bookmarks", "list", "--refresh", "--no-cache"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
    #[test]
    fn update_contract() {
        for args in [
            ["xcli", "update"].as_slice(),
            &["xcli", "update", "--check"],
            &["xcli", "update", "-y"],
            &["xcli", "update", "--yes"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert!(matches!(cli.command, Command::Update { .. }), "{args:?}");
        }
        // --check never installs, so combining it with --yes is contradictory.
        assert!(Cli::try_parse_from(["xcli", "update", "--check", "--yes"]).is_err());
        // Account/provider/pagination flags are rejected, not ignored.
        for flag in [
            "--account",
            "--connection",
            "--backend",
            "--max-pages",
            "--cursor",
        ] {
            assert!(
                Cli::try_parse_from(["xcli", "update", flag, "value"]).is_err(),
                "{flag}"
            );
        }
    }
}
