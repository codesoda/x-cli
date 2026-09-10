use serde::{Deserialize, Serialize};
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    InvalidInput,
    Unavailable,
    Permission,
    Authentication,
    RateLimit,
    Unsupported,
    ProtocolChanged,
    Network,
    IdentityMismatch,
    Storage,
    ConsentRequired,
}

/// Safe protocol metadata only; never contains upstream strings or payload fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub enum Diagnostic {
    Http { status: u16 },
    Json,
    GraphqlErrors { code: Option<u32> },
    ResponseRoot,
    Identity,
    Post,
    TimelineInstructions,
    TimelineInstruction,
    TimelineEntry,
    TimelineItem,
    Cursor,
}

/// Only static, reviewed messages may enter errors; never upstream bodies or headers.
#[derive(Debug, Clone, Serialize)]
pub struct Error {
    pub kind: Kind,
    pub message: &'static str,
    pub retry_after_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<Diagnostic>,
}
impl Error {
    pub fn new(kind: Kind, message: &'static str) -> Self {
        Self {
            kind,
            message,
            retry_after_seconds: None,
            diagnostic: None,
        }
    }
    /// Preserve the most specific classification as errors pass through outer parsers.
    pub fn at(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostic.get_or_insert(diagnostic);
        self
    }
    pub fn exit_code(&self) -> u8 {
        match self.kind {
            Kind::InvalidInput => 2,
            Kind::Unavailable => 3,
            Kind::Permission => 4,
            Kind::Authentication | Kind::ConsentRequired => 5,
            Kind::RateLimit => 6,
            Kind::Unsupported => 7,
            Kind::ProtocolChanged => 8,
            Kind::Network => 9,
            Kind::IdentityMismatch => 10,
            Kind::Storage => 11,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for Error {}
pub fn protocol() -> Error {
    Error::new(
        Kind::ProtocolChanged,
        "Upstream response does not match the supported protocol",
    )
}
pub fn storage() -> Error {
    Error::new(Kind::Storage, "Cannot safely read or write local state")
}
