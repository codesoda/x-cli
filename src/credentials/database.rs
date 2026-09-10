use super::{Session, crypto, unsupported};
use crate::error::{Error, Kind, Result};
use rusqlite::{Connection, OpenFlags, Transaction, types::ValueRef};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

const CHROME_EPOCH_SECONDS: u64 = 11_644_473_600;

pub(super) struct StoredPair {
    host: String,
    auth_token: Zeroizing<Vec<u8>>,
    ct0: Zeroizing<Vec<u8>>,
}

impl StoredPair {
    pub(super) fn decrypt(self, password: &[u8]) -> Result<Session> {
        let key = crypto::derive_key(password)?;
        let auth = crypto::decrypt_v24(&self.auth_token, &self.host, &key)?;
        let csrf = crypto::decrypt_v24(&self.ct0, &self.host, &key)?;
        Session::from_zeroizing(auth, csrf)
    }
}

fn database_error() -> Error {
    Error::new(
        Kind::Storage,
        "Cannot safely read Chrome cookies; close Chrome and retry",
    )
}

fn schema_error(error: rusqlite::Error) -> Error {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            database_error()
        }
        _ => unsupported(),
    }
}

fn missing_pair() -> Error {
    Error::new(
        Kind::Authentication,
        "No unambiguous supported X session in the selected profile",
    )
}

fn chrome_now() -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| database_error())?;
    let seconds = now
        .as_secs()
        .checked_add(CHROME_EPOCH_SECONDS)
        .ok_or_else(database_error)?;
    let micros = seconds
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(u64::from(now.subsec_micros())))
        .ok_or_else(database_error)?;
    i64::try_from(micros).map_err(|_| database_error())
}

pub(super) fn read(path: &Path) -> Result<StoredPair> {
    // No URI/immutable mode, backup, dump, or main-file copy. SQLite itself
    // coordinates the WAL snapshot. Missing/unusable WAL/SHM or locking errors
    // are surfaced, never retried with a less safe strategy.
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(|_| database_error())?;
    connection
        .busy_timeout(Duration::from_millis(250))
        .map_err(|_| database_error())?;
    connection
        .execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA temp_store=MEMORY;")
        .map_err(|_| database_error())?;
    let pair = read_transaction(&mut connection, chrome_now()?)?;
    connection.close().map_err(|_| database_error())?;
    Ok(pair)
}

fn read_transaction(connection: &mut Connection, now: i64) -> Result<StoredPair> {
    // The first schema/meta read pins the snapshot for both version and rows.
    let transaction = connection.transaction().map_err(|_| database_error())?;
    check_schema(&transaction)?;
    let pair = select_pair(&transaction, now)?;
    transaction.commit().map_err(|_| database_error())?;
    Ok(pair)
}

fn check_schema(transaction: &Transaction<'_>) -> Result<()> {
    // Do not accept an executable view standing in for a known table.
    for name in ["meta", "cookies"] {
        let kind: String = transaction
            .query_row(
                "SELECT type FROM sqlite_schema WHERE name = ?1 COLLATE BINARY",
                [name],
                |r| r.get(0),
            )
            .map_err(schema_error)?;
        if kind != "table" {
            return Err(unsupported());
        }
    }
    let mut statement = transaction.prepare(
        "SELECT key, value FROM meta WHERE key COLLATE BINARY IN ('version', 'last_compatible_version') LIMIT 3",
    ).map_err(schema_error)?;
    let mut rows = statement.query([]).map_err(|_| database_error())?;
    let (mut version, mut compatible) = (false, false);
    while let Some(row) = rows.next().map_err(|_| database_error())? {
        let key: String = row.get(0).map_err(|_| unsupported())?;
        let value = row.get_ref(1).map_err(|_| unsupported())?;
        if !matches!(value, ValueRef::Integer(24) | ValueRef::Text(b"24")) {
            return Err(unsupported());
        }
        match key.as_str() {
            "version" if !version => version = true,
            "last_compatible_version" if !compatible => compatible = true,
            _ => return Err(unsupported()),
        }
    }
    if !version || !compatible {
        return Err(unsupported());
    }
    Ok(())
}

fn select_pair(transaction: &Transaction<'_>, now: i64) -> Result<StoredPair> {
    // Deliberately include partition metadata in candidate rows rather than
    // hiding partition conflicts with top_frame_site_key=''. Ignore expired,
    // insecure, non-root, and non-X cookies without ever returning their values.
    // Session cookies are supported only with canonical zero expiry flags.
    let mut statement = transaction
        .prepare(
            "SELECT host_key, name, value, encrypted_value, top_frame_site_key,
                has_cross_site_ancestor, source_scheme, source_port
         FROM cookies
         WHERE host_key COLLATE BINARY IN ('.x.com', 'x.com')
           AND name COLLATE BINARY IN ('auth_token', 'ct0')
           AND path COLLATE BINARY = '/' AND is_secure = 1
           AND typeof(expires_utc) = 'integer'
           AND ((has_expires = 1 AND is_persistent = 1 AND expires_utc > ?1)
             OR (has_expires = 0 AND is_persistent = 0 AND expires_utc = 0))
         LIMIT 3",
        )
        .map_err(schema_error)?;
    let mut rows = statement.query([now]).map_err(|_| database_error())?;
    let mut host = None;
    let mut auth_token = None;
    let mut ct0 = None;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(|_| database_error())? {
        count += 1;
        if count > 2 {
            return Err(missing_pair());
        }
        let row_host: String = row.get(0).map_err(|_| unsupported())?;
        if host.as_ref().is_some_and(|host| host != &row_host) {
            // Never build a pair across host-only/domain jars, even when
            // both cookies would individually match a request to x.com.
            return Err(missing_pair());
        }
        host = Some(row_host);
        let name: String = row.get(1).map_err(|_| unsupported())?;
        let plaintext = row.get_ref(2).map_err(|_| unsupported())?;
        if !matches!(plaintext, ValueRef::Text(b"")) {
            // Plaintext and mixed plaintext/encrypted storage are not supported.
            return Err(unsupported());
        }
        let partition = row.get_ref(4).map_err(|_| unsupported())?;
        let ancestor: i64 = row.get(5).map_err(|_| unsupported())?;
        let scheme: i64 = row.get(6).map_err(|_| unsupported())?;
        let port: i64 = row.get(7).map_err(|_| unsupported())?;
        // Chromium serializes an absent partition key as ("", true), and
        // FromStorage ignores the ancestor bit when the site is empty.
        if !matches!(partition, ValueRef::Text(b""))
            || !matches!(ancestor, 0 | 1)
            || scheme != 2
            || !matches!(port, -1 | 443)
        {
            return Err(unsupported());
        }
        let encrypted = match row.get_ref(3).map_err(|_| unsupported())? {
            ValueRef::Blob(value)
                if value.len() <= crypto::MAX_ENCRYPTED
                    && value.len() > 3
                    && value.starts_with(b"v10")
                    && (value.len() - 3).is_multiple_of(16) =>
            {
                Zeroizing::new(value.to_vec())
            }
            _ => return Err(unsupported()),
        };
        match name.as_str() {
            "auth_token" if auth_token.is_none() => auth_token = Some(encrypted),
            "ct0" if ct0.is_none() => ct0 = Some(encrypted),
            _ => return Err(missing_pair()),
        }
    }
    match (host, auth_token, ct0) {
        (Some(host), Some(auth_token), Some(ct0)) => Ok(StoredPair {
            host,
            auth_token,
            ct0,
        }),
        _ => Err(missing_pair()),
    }
}

#[cfg(test)]
pub(super) mod tests;
