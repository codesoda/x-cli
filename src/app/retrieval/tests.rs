//! Synthetic orchestration integration tests, not wire/live interoperability tests.
use super::*;
use crate::{
    cli::Cli,
    credentials::Session,
    model::{Identity, ListInfo, Post},
    pagination::{ListsPage, Page, UsersPage},
    transport::{Request, Response},
};
use clap::Parser;
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};

mod boundaries;
mod flow;
mod public;

const ACTOR: &str = "123";
const OTHER: &str = "789";

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Load,
    Connect,
    Viewer,
    Users(Operation, String, u32, Option<String>),
    Metadata(String, String),
    Read(String, String),
}
type Events = Rc<RefCell<Vec<Event>>>;

struct SyntheticCredentials(Events);
impl CredentialProvider for SyntheticCredentials {
    fn load(&self, profile: &str, consent: bool) -> Result<Session> {
        assert_eq!(profile, "Default");
        assert!(consent);
        self.0.borrow_mut().push(Event::Load);
        Session::new("synthetic-auth".into(), "synthetic-csrf".into())
    }
}

struct NoAccess;
impl Transport for NoAccess {
    fn get(&self, _: Request) -> Result<Response> {
        panic!("unexpected transport request")
    }
}
impl CredentialProvider for NoAccess {
    fn load(&self, _: &str, _: bool) -> Result<Session> {
        panic!("unexpected credential load")
    }
}
impl GraphFactory for NoAccess {
    type Client<'a> = FakeReadGraph<'a>;
    fn connect<'a>(&'a self, _: &'a dyn Transport, _: &'a Session) -> Result<Self::Client<'a>> {
        panic!("unexpected graph construction")
    }
}

struct FakeFactory {
    events: Events,
    bootstrap_error: Option<Kind>,
    viewer: Result<Identity>,
    pages: RefCell<VecDeque<Result<UsersPage>>>,
}
impl FakeFactory {
    fn new(events: Events) -> Self {
        Self {
            events,
            bootstrap_error: None,
            viewer: Ok(identity(ACTOR, "fixture")),
            pages: RefCell::new(VecDeque::new()),
        }
    }
    fn page(&self, cursor: Option<&str>) {
        self.pages.borrow_mut().push_back(Ok(UsersPage {
            users: vec![identity(OTHER, "other_fixture")],
            next_cursor: cursor.map(str::to_owned),
            warnings: vec![],
        }));
    }
}
impl GraphFactory for FakeFactory {
    type Client<'a> = FakeReadGraph<'a>;
    fn connect<'a>(&'a self, _: &'a dyn Transport, _: &'a Session) -> Result<Self::Client<'a>> {
        self.events.borrow_mut().push(Event::Connect);
        if let Some(kind) = self.bootstrap_error {
            return Err(Error::new(kind, "Synthetic bootstrap failure"));
        }
        Ok(FakeReadGraph(self))
    }
}
struct FakeReadGraph<'a>(&'a FakeFactory);
impl ReadGraph for FakeReadGraph<'_> {
    fn identity(&self) -> Result<Identity> {
        self.0.events.borrow_mut().push(Event::Viewer);
        self.0.viewer.clone()
    }
    fn user(&self, _: &str) -> Result<Identity> {
        panic!("unexpected user lookup")
    }
    fn read(&self, id: &str, account: &str) -> Result<Output> {
        self.0
            .events
            .borrow_mut()
            .push(Event::Read(id.into(), account.into()));
        let mut out = Output::new("graphql", Some(account.into()));
        out.posts.push(Post {
            id: id.into(),
            author: identity(OTHER, "other_fixture"),
            text: "Synthetic post".into(),
            created_at: None,
            parent_id: None,
            parent_known: true,
            url: format!("https://x.com/other_fixture/status/{id}"),
        });
        out.complete = true;
        out.pages = 1;
        out.stop_reason = "single_post".into();
        Ok(out)
    }
    fn page(&self, _: Operation, _: &str, _: u32, _: Option<&str>) -> Result<Page> {
        panic!("unexpected post page")
    }
    fn users_page(
        &self,
        op: Operation,
        target: &str,
        count: u32,
        cursor: Option<&str>,
    ) -> Result<UsersPage> {
        self.0.events.borrow_mut().push(Event::Users(
            op,
            target.into(),
            count,
            cursor.map(str::to_owned),
        ));
        self.0
            .pages
            .borrow_mut()
            .pop_front()
            .expect("unexpected user page")
    }
    fn list_metadata(&self, id: &str, account: &str) -> Result<Output> {
        self.0
            .events
            .borrow_mut()
            .push(Event::Metadata(id.into(), account.into()));
        let mut out = Output::new("graphql", Some(account.into()));
        out.lists = Some(vec![ListInfo {
            id: id.into(),
            name: "Synthetic list".into(),
            description: None,
            visibility: None,
            owner: Some(identity(OTHER, "other_fixture")),
            subscribed: None,
            pinned: None,
            is_member: None,
            management_sections: vec![],
            url: format!("https://x.com/i/lists/{id}"),
        }]);
        out.complete = true;
        out.pages = 1;
        out.stop_reason = "single_list".into();
        Ok(out)
    }
    fn lists_page(&self, _: &str, _: u32, _: Option<&str>) -> Result<ListsPage> {
        panic!("unexpected list page")
    }
}

struct Harness {
    _temp: tempfile::TempDir,
    root: PathBuf,
    cache: Cache,
    events: Events,
}
impl Harness {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        Config::update(&root, |config| {
            config.add(
                "Default".into(),
                Some("work".into()),
                identity(ACTOR, "fixture"),
            )?;
            config.add(
                "Profile 1".into(),
                Some("other".into()),
                identity(OTHER, "other_fixture"),
            )?;
            config.set_default("work")
        })
        .unwrap();
        Self {
            cache: Cache::new(root.join("cache")),
            root,
            _temp: temp,
            events: Rc::default(),
        }
    }
    fn run(&self, args: &[&str], factory: &impl GraphFactory) -> Result<Output> {
        self.run_with(
            args,
            factory,
            &NoAccess,
            &SyntheticCredentials(self.events.clone()),
        )
    }
    fn run_with(
        &self,
        args: &[&str],
        factory: &impl GraphFactory,
        transport: &dyn Transport,
        credentials: &dyn CredentialProvider,
    ) -> Result<Output> {
        let cli = cli(args);
        let (access, task) = Task::from_command(&cli.command)?;
        let cache = Cache::new(self.root.join("cache"));
        let result = execute_with_factory(
            &self.root,
            &cache,
            access,
            task,
            transport,
            credentials,
            factory,
        )?;
        Ok(serde_json::from_value(result).unwrap())
    }
    fn seed(&self, args: &[&str], output: &Output) {
        self.cache
            .put(
                &output.provenance.backend,
                output.provenance.account_id.as_deref(),
                &key(args),
                output,
            )
            .unwrap();
    }
    fn stored(&self, args: &[&str], backend: &str, account: Option<&str>) -> Option<Output> {
        self.cache.get(backend, account, &key(args), 86400).unwrap()
    }
    fn expect_events(&self, expected: Vec<Event>) {
        assert_eq!(self.events.take(), expected);
    }
    fn corrupt_only_content(&self) {
        // This helper is used before any other scope exists: enumerate only this
        // synthetic content directory, never config/cooldowns or real state.
        let files: Vec<_> = std::fs::read_dir(self.root.join("cache/content"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].extension().unwrap(), "json");
        std::fs::write(&files[0], b"{invalid synthetic content JSON").unwrap();
    }
}
fn identity(id: &str, handle: &str) -> Identity {
    Identity {
        id: id.into(),
        handle: handle.into(),
    }
}
fn cli(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("xcli").chain(args.iter().copied())).unwrap()
}
fn key(args: &[&str]) -> String {
    Task::from_command(&cli(args).command).unwrap().1.key()
}
fn verified() -> Vec<Event> {
    vec![Event::Load, Event::Connect, Event::Viewer]
}
fn users_event(op: Operation, count: u32, cursor: Option<&str>) -> Event {
    Event::Users(op, ACTOR.into(), count, cursor.map(str::to_owned))
}
fn assert_users(out: &Output, cache: &str) {
    assert_eq!(
        out.users.as_ref().unwrap(),
        &[identity(OTHER, "other_fixture")]
    );
    assert!(out.posts.is_empty() && out.lists.is_none());
    assert_eq!(out.provenance.backend, "graphql");
    assert_eq!(out.provenance.account_id.as_deref(), Some(ACTOR));
    assert_eq!(out.provenance.cache, cache);
    assert!(!out.complete);
    assert_eq!(out.pages, 1);
}
