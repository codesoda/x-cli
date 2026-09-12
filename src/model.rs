use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub id: String,
    pub handle: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub author: Identity,
    pub text: String,
    pub created_at: Option<String>,
    pub parent_id: Option<String>,
    /// False means missing parent data cannot be interpreted as a root post.
    pub parent_known: bool,
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub backend: String,
    pub account_id: Option<String>,
    /// Unix seconds, recording local retrieval time, not source freshness guarantees.
    pub retrieved_at: u64,
    pub cache: String,
    pub age_seconds: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub posts: Vec<Post>,
    /// Present (including an empty array) only for user collections. Post output stays compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<Identity>>,
    pub provenance: Provenance,
    pub complete: bool,
    #[serde(default)]
    pub parent_chain_complete: Option<bool>,
    #[serde(default)]
    pub replies_complete: Option<bool>,
    #[serde(default)]
    pub request_failed: bool,
    pub stop_reason: String,
    pub next_cursor: Option<String>,
    pub pages: u32,
    pub warnings: Vec<String>,
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
impl Output {
    pub fn new(backend: &str, account_id: Option<String>) -> Self {
        Self {
            posts: vec![],
            users: None,
            provenance: Provenance {
                backend: backend.into(),
                account_id,
                retrieved_at: now(),
                cache: "miss".into(),
                age_seconds: 0,
            },
            complete: false,
            parent_chain_complete: None,
            replies_complete: None,
            request_failed: false,
            stop_reason: "unknown".into(),
            next_cursor: None,
            pages: 0,
            warnings: vec![],
        }
    }
}
