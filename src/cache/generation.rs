//! One bounded, cache-root-wide epoch; never held across upstream requests.
use super::*;
use serde::{Deserialize, Serialize};

const METADATA: &str = "generation.json";
const VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    version: u32,
    generation: u64,
}

/// Opaque write eligibility captured before a fetch, bound to this cache path.
/// It is not upstream freshness evidence or authorization for an account.
#[derive(Debug, Clone)]
pub struct CacheGeneration {
    root: PathBuf,
    value: u64,
}

/// A generation mismatch is not an IO or upstream request failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalPut {
    /// The normal bounded put policy ran, including removal of an oversized hit.
    Applied,
    /// Invalidation occurred after capture; no content was touched by this put.
    GenerationChanged,
}

impl Cache {
    /// Capture under .cache.lock, releasing it before returning. Missing metadata
    /// means zero; corrupt/unsupported metadata is a storage error, never reset.
    pub fn capture_generation(&self) -> Result<CacheGeneration> {
        state::with_lock(&self.root.join(".cache.lock"), || {
            Ok(CacheGeneration {
                root: self.root.clone(),
                value: self.generation_locked()?,
            })
        })
    }

    /// Compare and write under the same lock as invalidation. Foreign-root tokens
    /// fail with Storage. Existing scope, timestamp and capacity guards still
    /// apply to matching writes. Older binaries and direct `put` are not fenced.
    pub fn put_if_generation(
        &self,
        generation: &CacheGeneration,
        backend: &str,
        account: Option<&str>,
        key: &str,
        output: &Output,
    ) -> Result<ConditionalPut> {
        state::with_lock(&self.root.join(".cache.lock"), || {
            if generation.root != self.root {
                return Err(storage());
            }
            if generation.value != self.generation_locked()? {
                return Ok(ConditionalPut::GenerationChanged);
            }
            self.put_locked(backend, account, key, output)?;
            Ok(ConditionalPut::Applied)
        })
    }

    fn generation_locked(&self) -> Result<u64> {
        let Some(metadata) = state::read_json::<Metadata>(&self.root.join(METADATA))? else {
            return Ok(0);
        };
        if metadata.version != VERSION {
            return Err(storage());
        }
        Ok(metadata.generation)
    }

    pub(super) fn advance_generation_locked(&self) -> Result<()> {
        let generation = self
            .generation_locked()?
            .checked_add(1)
            .ok_or_else(storage)?;
        state::write_json(
            &self.root.join(METADATA),
            &Metadata {
                version: VERSION,
                generation,
            },
        )
    }
}
