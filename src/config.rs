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
mod tests {
    use super::*;
    fn identity(id: &str, handle: &str) -> Identity {
        Identity {
            id: id.into(),
            handle: handle.into(),
        }
    }
    fn add(c: &mut Config, profile: &str, alias: &str, id: &str, handle: &str) -> Connection {
        c.add(profile.into(), Some(alias.into()), identity(id, handle))
            .unwrap()
    }
    #[test]
    fn aliases_handles_defaults_and_renames() {
        let mut c = Config::default();
        let a = add(&mut c, "Default", "work", "1", "Alice");
        let b = add(&mut c, "Profile 1", "alice", "2", "Bob");
        assert_eq!(c.resolve(Some("@ALICE"), None).unwrap().id, a.id);
        assert_eq!(c.resolve(Some("alice"), None).unwrap().id, b.id);
        assert_eq!(c.resolve(None, None).unwrap().id, a.id);
        assert!(c.resolve(Some("Bob"), None).is_err());
        c.set_default("alice").unwrap();
        assert_eq!(c.default.as_deref(), Some(b.id.as_str()));
        c.rename(&b.id, Some("personal".into())).unwrap();
        assert_eq!(c.resolve(None, None).unwrap().id, b.id);
        assert!(c.rename(&b.id, Some("WORK".into())).is_err());
        assert_eq!(c.resolve(Some("personal"), None).unwrap().id, b.id);
        assert!(c.resolve(Some("work"), Some(&b.id)).is_err());
        assert!(c.set_default("missing").is_err());
        assert_eq!(c.default.as_deref(), Some(b.id.as_str()));
    }
    #[test]
    fn same_id_aliases_do_not_bypass_connection_ambiguity() {
        let mut c = Config::default();
        let a = add(&mut c, "Default", "first", "1", "alice");
        let b = add(&mut c, "Profile 1", "second", "1", "renamed");
        for selector in [None, Some("first"), Some("second"), Some("@alice")] {
            assert!(c.resolve(selector, None).is_err());
        }
        assert_eq!(c.resolve(Some("first"), Some(&b.id)).unwrap().id, b.id);
        c.prefer(&a.id).unwrap();
        assert_eq!(c.resolve(Some("second"), None).unwrap().id, a.id);
        c.prefer(&b.id).unwrap();
        assert!(!c.connections[0].preferred);
        assert_eq!(c.resolve(Some("@alice"), None).unwrap().id, b.id);
        assert_eq!(c.resolve(None, Some(&a.id)).unwrap().id, a.id);
    }
    #[test]
    fn reject_invalid_profiles_aliases_and_duplicates_without_mutating() {
        let mut c = Config::default();
        for profile in [
            "../Default",
            "/Default",
            "Profile 1/sub",
            "Guest Profile",
            "Profile -1",
            "Profile 01",
            "Profile ",
        ] {
            assert!(c.add(profile.into(), None, identity("1", "a")).is_err());
        }
        for alias in ["@alice", "", "a/b", "has space"] {
            assert!(
                c.add("Default".into(), Some(alias.into()), identity("1", "a"))
                    .is_err()
            );
        }
        assert!(c.connections.is_empty());
        add(&mut c, "Default", "work", "1", "a");
        assert!(
            c.add("Profile 1".into(), Some("WORK".into()), identity("2", "b"))
                .is_err()
        );
        assert!(c.add("Default".into(), None, identity("2", "b")).is_err());
        assert_eq!(c.connections.len(), 1);
    }
    #[test]
    fn identity_verification_uses_id_not_handle() {
        assert!(verify_identity(&identity("1", "old"), &identity("1", "new")).is_ok());
        assert_eq!(
            verify_identity(&identity("1", "same"), &identity("2", "same"))
                .unwrap_err()
                .kind,
            Kind::IdentityMismatch
        );
    }
    #[test]
    fn reused_handle_requires_connection() {
        let mut c = Config::default();
        let a = add(&mut c, "Default", "a", "1", "same");
        add(&mut c, "Profile 1", "b", "2", "same");
        assert!(c.resolve(Some("@same"), None).is_err());
        assert_eq!(c.resolve(Some("@same"), Some(&a.id)).unwrap().id, a.id);
    }
    #[test]
    fn concurrent_updates_and_explicit_removal() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap();
        std::thread::scope(|scope| {
            for i in 1..=4 {
                let root = &root;
                scope.spawn(move || {
                    Config::update(root, |c| {
                        c.add(
                            format!("Profile {i}"),
                            Some(format!("a{i}")),
                            identity(&i.to_string(), &format!("user{i}")),
                        )
                    })
                    .unwrap()
                });
            }
        });
        let config = Config::load(&root).unwrap();
        assert_eq!(config.connections.len(), 4);
        let default = config.default.unwrap();
        Config::update(&root, |c| c.remove(&default)).unwrap();
        let config = Config::load(&root).unwrap();
        assert_eq!(config.connections.len(), 3);
        assert!(config.default.is_none());
        assert!(config.resolve(None, Some(&default)).is_err());
    }
    #[test]
    fn roundtrip_and_validate_loaded_config() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap();
        let mut c = Config::load(&root).unwrap();
        let a = add(&mut c, "Default", "work", "1", "@alice");
        c.save(&root).unwrap();
        assert_eq!(
            Config::load(&root).unwrap().resolve(None, None).unwrap().id,
            a.id
        );
        c.default = Some("missing".into());
        state::write_json(&root.join("config.json"), &c).unwrap();
        assert!(Config::load(&root).is_err());
        c.default = None;
        c.connections[0].profile = "../escape".into();
        state::write_json(&root.join("config.json"), &c).unwrap();
        assert!(Config::load(&root).is_err());
        assert!(c.save(&root).is_err());
    }
}
