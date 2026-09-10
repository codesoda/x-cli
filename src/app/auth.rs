use super::rate_result;
use crate::{
    cache::Cache,
    cli::AuthCommand,
    config::Config,
    credentials::{CredentialProvider, Profile},
    error::{Error, Kind, Result, storage},
    model::now,
    providers::graphql::Graphql,
    transport::Transport,
};
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn execute(
    command: &AuthCommand,
    root: &Path,
    cache: &Cache,
    t: &dyn Transport,
    credentials: &dyn CredentialProvider,
    discover: impl FnOnce() -> Result<Vec<Profile>>,
) -> Result<Value> {
    if matches!(command, AuthCommand::Discover) {
        return serde_json::to_value(discover()?).map_err(|_| storage());
    }
    let config = Config::load(root)?;
    match command {
        AuthCommand::List => return serde_json::to_value(config).map_err(|_| storage()),
        AuthCommand::Add {
            profile,
            alias,
            consent,
            ..
        } => {
            if !consent {
                return Err(Error::new(
                    Kind::ConsentRequired,
                    "Connecting requires --consent for local X cookie/Keychain access and a read-only X identity request",
                ));
            }
            // Validate connection syntax and uniqueness before any credential access.
            let mut trial = config.clone();
            trial.add(
                profile.clone(),
                alias.clone(),
                crate::model::Identity {
                    id: "1".into(),
                    handle: "validation".into(),
                },
            )?;
            cache.check_cooldown("graphql", None)?;
            let result = (|| {
                let session = credentials.load(profile, true)?;
                let graph = Graphql::new(t, &session)?;
                graph.identity()
            })();
            let identity = rate_result(cache, "graphql", None, result)?;
            let connection = Config::update(root, |current| {
                current.add(profile.clone(), alias.clone(), identity)
            })?;
            return Ok(
                json!({"connection":connection,"verified_at":now(),"warnings":["Connection permits on-demand session loading for read-only requests. Browser cookies are account-level credentials."]}),
            );
        }
        AuthCommand::Discover => unreachable!(),
        _ => {}
    }
    Config::update(root, |current| {
        match command {
            AuthCommand::Remove { connection } => current.remove(connection)?,
            AuthCommand::Default { account } => current.set_default(account)?,
            AuthCommand::Prefer { connection } => current.prefer(connection)?,
            AuthCommand::Rename { connection, alias } => {
                current.rename(connection, alias.clone())?
            }
            _ => unreachable!(),
        }
        serde_json::to_value(current).map_err(|_| storage())
    })
}
