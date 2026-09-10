use serde::Serialize;
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

/// Only static, reviewed messages may enter errors; never upstream bodies or headers.
#[derive(Debug, Clone, Serialize)]
pub struct Error {
    pub kind: Kind,
    pub message: &'static str,
    pub retry_after_seconds: Option<u64>,
}
impl Error {
    pub fn new(kind: Kind, message: &'static str) -> Self {
        Self {
            kind,
            message,
            retry_after_seconds: None,
        }
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
