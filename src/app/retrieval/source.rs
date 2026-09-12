//! Private read-only construction seam. Production bootstrap and wire validation
//! remain in Graphql; tests substitute normalized reads, not authorization/hash checks.
use crate::{
    credentials::Session,
    error::Result,
    model::{Identity, Output},
    pagination::{ListsPage, Page, UsersPage},
    providers::{graphql::Graphql, operations::Operation},
    transport::Transport,
};

pub(super) trait ReadGraph {
    fn identity(&self) -> Result<Identity>;
    fn user(&self, handle: &str) -> Result<Identity>;
    fn read(&self, id: &str, account: &str) -> Result<Output>;
    fn page(&self, op: Operation, target: &str, count: u32, cursor: Option<&str>) -> Result<Page>;
    fn users_page(
        &self,
        op: Operation,
        target_id: &str,
        count: u32,
        cursor: Option<&str>,
    ) -> Result<UsersPage>;
    fn list_metadata(&self, id: &str, account: &str) -> Result<Output>;
    fn lists_page(&self, actor: &str, count: u32, cursor: Option<&str>) -> Result<ListsPage>;
}

pub(super) trait GraphFactory {
    type Client<'a>: ReadGraph
    where
        Self: 'a;

    fn connect<'a>(
        &'a self,
        transport: &'a dyn Transport,
        session: &'a Session,
    ) -> Result<Self::Client<'a>>;
}

pub(super) struct DefaultFactory;
impl GraphFactory for DefaultFactory {
    type Client<'a> = Graphql<'a>;

    fn connect<'a>(
        &'a self,
        transport: &'a dyn Transport,
        session: &'a Session,
    ) -> Result<Self::Client<'a>> {
        Graphql::new(transport, session)
    }
}

impl ReadGraph for Graphql<'_> {
    fn identity(&self) -> Result<Identity> {
        Graphql::identity(self)
    }
    fn user(&self, handle: &str) -> Result<Identity> {
        Graphql::user(self, handle)
    }
    fn read(&self, id: &str, account: &str) -> Result<Output> {
        Graphql::read(self, id, account)
    }
    fn page(&self, op: Operation, target: &str, count: u32, cursor: Option<&str>) -> Result<Page> {
        Graphql::page(self, op, target, count, cursor)
    }
    fn users_page(
        &self,
        op: Operation,
        target_id: &str,
        count: u32,
        cursor: Option<&str>,
    ) -> Result<UsersPage> {
        Graphql::users_page(self, op, target_id, count, cursor)
    }
    fn list_metadata(&self, id: &str, account: &str) -> Result<Output> {
        Graphql::list_metadata(self, id, account)
    }
    fn lists_page(&self, actor: &str, count: u32, cursor: Option<&str>) -> Result<ListsPage> {
        Graphql::lists_page(self, actor, count, cursor)
    }
}
