//! Non-secret browser connection metadata. Account identity is always the stable ID.
use crate::{
    error::{Error, Kind, Result},
    input,
    model::Identity,
    state,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

/// Reviewed public-provider origin; arbitrary credential destinations are not configurable.
pub const FXTWITTER_API_ORIGIN: &str = "https://api.fxtwitter.com";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub connections: Vec<Connection>,
    /// A local connection ID, never a mutable handle or alias.
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub profile: String,
    pub alias: Option<String>,
    pub identity: Identity,
    pub preferred: bool,
}

fn invalid(message: &'static str) -> Error {
    Error::new(Kind::InvalidInput, message)
}

fn valid_profile(profile: &str) -> bool {
    profile == "Default"
        || profile.strip_prefix("Profile ").is_some_and(|n| {
            !n.is_empty()
                && n.len() <= 10
                && n.bytes().all(|b| b.is_ascii_digit())
                && n.parse::<u32>().is_ok_and(|v| v.to_string() == n)
        })
}
fn validate_alias(alias: Option<&str>) -> Result<()> {
    if alias.is_some_and(|a| {
        a.is_empty()
            || a.len() > 64
            || !a
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    }) {
        return Err(invalid(
            "Aliases must contain only letters, digits, underscores or hyphens and cannot start with @",
        ));
    }
    Ok(())
}

impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let config: Self = state::read_json(&root.join("config.json"))?.unwrap_or_default();
        config.validate()?;
        Ok(config)
    }
    /// Serialize connection registration/changes without losing concurrent updates.
    pub fn update<T>(root: &Path, change: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        state::with_lock(&root.join(".config.lock"), || {
            let mut config = Self::load(root)?;
            let result = change(&mut config)?;
            config.save(root)?;
            Ok(result)
        })
    }
    pub fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        state::write_json(&root.join("config.json"), self)
    }
    fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        let mut profiles = HashSet::new();
        let mut aliases = HashSet::new();
        let mut preferred = HashSet::new();
        for c in &self.connections {
            if c.id.is_empty()
                || c.id.len() > 128
                || !c
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                || !ids.insert(c.id.as_str())
            {
                return Err(invalid("Invalid or duplicate local connection ID"));
            }
            if !valid_profile(&c.profile) || !profiles.insert(c.profile.as_str()) {
                return Err(invalid("Invalid or duplicate Chrome profile"));
            }
            validate_alias(c.alias.as_deref())?;
            if c.alias
                .as_ref()
                .is_some_and(|a| !aliases.insert(a.to_ascii_lowercase()))
            {
                return Err(invalid("Connection aliases must be unique"));
            }
            input::id(&c.identity.id)?;
            input::handle(&c.identity.handle)?;
            if c.identity.handle.starts_with('@') {
                return Err(invalid("Stored handles must not include @"));
            }
            if c.preferred && !preferred.insert(c.identity.id.as_str()) {
                return Err(invalid("An account can have only one preferred connection"));
            }
        }
        if self
            .default
            .as_ref()
            .is_some_and(|d| !ids.contains(d.as_str()))
        {
            return Err(invalid("Default refers to an unknown connection"));
        }
        Ok(())
    }
    pub fn add(
        &mut self,
        profile: String,
        alias: Option<String>,
        mut identity: Identity,
    ) -> Result<Connection> {
        self.validate()?;
        identity.handle = input::handle(&identity.handle)?;
        // Build and validate a candidate so failed changes leave the old config intact.
        let mut candidate = self.clone();
        let mut n = 1u64;
        let id = loop {
            let id = format!("connection-{n}");
            if !self.connections.iter().any(|c| c.id == id) {
                break id;
            }
            n = n
                .checked_add(1)
                .ok_or_else(|| invalid("Too many connections"))?;
        };
        let connection = Connection {
            id,
            profile,
            alias,
            identity,
            preferred: false,
        };
        if candidate.connections.is_empty() {
            candidate.default = Some(connection.id.clone());
        }
        candidate.connections.push(connection.clone());
        candidate.validate()?;
        *self = candidate;
        Ok(connection)
    }
    fn by_connection(&self, id: &str) -> Result<&Connection> {
        self.connections
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| invalid("Unknown local connection ID"))
    }
    fn selector_identity(&self, selector: &str) -> Result<&str> {
        if selector.starts_with('@') {
            let handle = input::handle(selector)?;
            let matches: Vec<_> = self
                .connections
                .iter()
                .filter(|c| c.identity.handle.eq_ignore_ascii_case(&handle))
                .collect();
            let first = matches
                .first()
                .ok_or_else(|| invalid("Unknown account handle"))?;
            if matches.iter().any(|c| c.identity.id != first.identity.id) {
                return Err(invalid(
                    "Handle maps to multiple stable accounts; use --connection",
                ));
            }
            Ok(&first.identity.id)
        } else {
            let c = self
                .connections
                .iter()
                .find(|c| {
                    c.alias
                        .as_ref()
                        .is_some_and(|a| a.eq_ignore_ascii_case(selector))
                })
                .ok_or_else(|| invalid("Unknown account alias; handles require @"))?;
            Ok(&c.identity.id)
        }
    }
    pub fn resolve(&self, selector: Option<&str>, connection: Option<&str>) -> Result<&Connection> {
        self.validate()?;
        if let Some(id) = connection {
            let chosen = self.by_connection(id)?;
            if let Some(selector) = selector {
                // An explicit connection disambiguates even a stale/reused handle.
                let matches = if selector.starts_with('@') {
                    chosen
                        .identity
                        .handle
                        .eq_ignore_ascii_case(&input::handle(selector)?)
                } else {
                    self.selector_identity(selector)? == chosen.identity.id
                };
                if !matches {
                    return Err(invalid(
                        "Account selector and connection refer to different identities",
                    ));
                }
            }
            return Ok(chosen);
        }
        let identity = if let Some(selector) = selector {
            self.selector_identity(selector)?
        } else if let Some(default) = &self.default {
            &self.by_connection(default)?.identity.id
        } else {
            let first = self
                .connections
                .first()
                .ok_or_else(|| invalid("No connected accounts"))?;
            if self
                .connections
                .iter()
                .any(|c| c.identity.id != first.identity.id)
            {
                return Err(invalid("Select an account or configure a default"));
            }
            &first.identity.id
        };
        let mut candidates = self
            .connections
            .iter()
            .filter(|c| c.identity.id == identity);
        let first = candidates
            .next()
            .ok_or_else(|| invalid("Unknown stable account"))?;
        if let Some(preferred) = self
            .connections
            .iter()
            .find(|c| c.identity.id == identity && c.preferred)
        {
            return Ok(preferred);
        }
        if candidates.next().is_some() {
            return Err(invalid(
                "Account has multiple connections; set a preference or use --connection",
            ));
        }
        Ok(first)
    }
    pub fn set_default(&mut self, selector: &str) -> Result<()> {
        self.default = Some(self.resolve(Some(selector), None)?.id.clone());
        Ok(())
    }
    pub fn remove(&mut self, connection: &str) -> Result<()> {
        self.validate()?;
        self.by_connection(connection)?;
        self.connections.retain(|c| c.id != connection);
        if self.default.as_deref() == Some(connection) {
            self.default = None;
        }
        Ok(())
    }
    pub fn prefer(&mut self, connection: &str) -> Result<()> {
        self.validate()?;
        let identity = self.by_connection(connection)?.identity.id.clone();
        for c in &mut self.connections {
            if c.identity.id == identity {
                c.preferred = c.id == connection;
            }
        }
        Ok(())
    }
    pub fn rename(&mut self, connection: &str, alias: Option<String>) -> Result<()> {
        self.validate()?;
        self.by_connection(connection)?;
        let mut candidate = self.clone();
        let chosen = candidate
            .connections
            .iter_mut()
            .find(|c| c.id == connection)
            .ok_or_else(|| invalid("Unknown local connection ID"))?;
        chosen.alias = alias;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

pub fn verify_identity(expected: &Identity, actual: &Identity) -> Result<()> {
    if expected.id != actual.id || expected.id.is_empty() {
        return Err(Error::new(
            Kind::IdentityMismatch,
            "Browser account does not match the connected stable identity",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
