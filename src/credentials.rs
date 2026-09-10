//! Explicit-consent Chrome session access. Never serialize a `Session` or log
//! the headers it produces; the caller must restrict them to HTTPS x.com.
//!
//! Storage compatibility is not authenticated identity. Verify Viewer before
//! associating the session with a persisted account reference.

mod chrome;
#[cfg(any(target_os = "macos", test))]
mod crypto;
#[cfg(any(target_os = "macos", test))]
mod database;

use crate::error::{Error, Kind, Result};
pub use chrome::Chrome;
use serde::Serialize;
use std::fmt;
use zeroize::Zeroizing;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub directory: String,
}

pub trait CredentialProvider {
    /// Consent is required on every load, before filesystem or Keychain access.
    fn load(&self, profile: &str, consent: bool) -> Result<Session>;
}

/// Account-level authority, held only in zeroizing buffers. Deliberately does
/// not implement Serialize, Display, or Clone.
pub struct Session {
    auth_token: Zeroizing<String>,
    ct0: Zeroizing<String>,
}

impl Session {
    pub fn new(auth_token: String, ct0: String) -> Result<Self> {
        Self::from_zeroizing(Zeroizing::new(auth_token), Zeroizing::new(ct0))
    }

    fn from_zeroizing(auth_token: Zeroizing<String>, ct0: Zeroizing<String>) -> Result<Self> {
        if !valid_cookie(&auth_token) || !valid_cookie(&ct0) {
            return Err(Error::new(
                Kind::Authentication,
                "Invalid session cookie encoding",
            ));
        }
        Ok(Self { auth_token, ct0 })
    }

    /// Only private session headers. Public authorization belongs to the
    /// GraphQL layer. Zeroizing header values are still readable via Debug;
    /// callers must never log this result or copy it to persistent storage.
    pub fn headers(&self) -> Vec<(String, Zeroizing<String>)> {
        let mut cookie = Zeroizing::new(String::with_capacity(
            "auth_token=; ct0=".len() + self.auth_token.len() + self.ct0.len(),
        ));
        cookie.push_str("auth_token=");
        cookie.push_str(&self.auth_token);
        cookie.push_str("; ct0=");
        cookie.push_str(&self.ct0);
        vec![
            ("cookie".into(), cookie),
            ("x-csrf-token".into(), Zeroizing::new(self.ct0.to_string())),
        ]
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Session { auth_token: [REDACTED], ct0: [REDACTED] }")
    }
}

// RFC 6265 cookie-octet also excludes header injection and cookie separators.
// A bound prevents unexpectedly large values from reaching an HTTP client.
fn valid_cookie(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value
            .bytes()
            .all(|b| matches!(b, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}

fn unsupported() -> Error {
    Error::new(
        Kind::Unsupported,
        "Unsupported Chrome cookie storage or protection",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_headers_are_minimal_and_debug_is_redacted() {
        let session = Session::new("synthetic-auth".into(), "synthetic-csrf".into()).unwrap();
        let headers = session.headers();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].0, "cookie");
        assert_eq!(
            &*headers[0].1,
            "auth_token=synthetic-auth; ct0=synthetic-csrf"
        );
        assert_eq!(headers[1].0, "x-csrf-token");
        assert_eq!(&*headers[1].1, "synthetic-csrf");
        assert_eq!(
            format!("{session:?}"),
            "Session { auth_token: [REDACTED], ct0: [REDACTED] }"
        );
    }

    #[test]
    fn rejects_empty_oversize_and_all_non_cookie_octets() {
        for b in 0u8..=127 {
            let s = String::from_utf8(vec![b]).unwrap();
            let expected =
                matches!(b, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e);
            assert_eq!(Session::new(s.clone(), "ok".into()).is_ok(), expected);
            assert_eq!(Session::new("ok".into(), s).is_ok(), expected);
        }
        for s in [
            String::new(),
            "a".repeat(4097),
            "café".into(),
            "a\r\nx-evil: true".into(),
        ] {
            assert!(Session::new(s, "ok".into()).is_err());
        }
    }
}
