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
pub(super) mod tests {
    use super::*;
    use rusqlite::params;

    pub(crate) const PASSWORD: &[u8] = b"synthetic-password";
    const NOW: i64 = 13_000_000_000_000_000;

    pub(crate) fn schema(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE meta(key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR);
             INSERT INTO meta VALUES ('version','24'), ('last_compatible_version','24');
             CREATE TABLE cookies (
               host_key TEXT NOT NULL, name TEXT NOT NULL, value TEXT NOT NULL DEFAULT '',
               encrypted_value BLOB NOT NULL, path TEXT NOT NULL DEFAULT '/',
               top_frame_site_key TEXT NOT NULL DEFAULT '',
               has_cross_site_ancestor INTEGER NOT NULL DEFAULT 1,
               source_scheme INTEGER NOT NULL DEFAULT 2, source_port INTEGER NOT NULL DEFAULT 443,
               expires_utc INTEGER NOT NULL DEFAULT 0, has_expires INTEGER NOT NULL DEFAULT 0,
               is_persistent INTEGER NOT NULL DEFAULT 0, is_secure INTEGER NOT NULL DEFAULT 1
             );",
            )
            .unwrap();
    }

    pub(crate) fn insert(connection: &Connection, host: &str, name: &str, value: &[u8]) {
        let key = crypto::derive_key(PASSWORD).unwrap();
        let encrypted = crypto::encrypt_fixture(host, value, &key);
        connection
            .execute(
                "INSERT INTO cookies(host_key,name,encrypted_value) VALUES (?1,?2,?3)",
                params![host, name, encrypted],
            )
            .unwrap();
    }

    pub(crate) fn pair(connection: &Connection, host: &str) {
        insert(connection, host, "auth_token", b"synthetic-auth");
        insert(connection, host, "ct0", b"synthetic-csrf");
    }

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        schema(&connection);
        pair(&connection, ".x.com");
        connection
    }

    fn result_kind(connection: &mut Connection) -> Kind {
        match read_transaction(connection, NOW) {
            Ok(_) => panic!("expected rejection"),
            Err(e) => e.kind,
        }
    }

    #[test]
    fn synthetic_schema24_pair_decrypts_with_real_ancestor_default() {
        for host in [".x.com", "x.com"] {
            let mut connection = Connection::open_in_memory().unwrap();
            schema(&connection);
            pair(&connection, host);
            let pair = read_transaction(&mut connection, NOW).unwrap();
            let session = pair.decrypt(PASSWORD).unwrap();
            assert_eq!(
                &*session.headers()[0].1,
                "auth_token=synthetic-auth; ct0=synthetic-csrf"
            );
        }
    }

    #[test]
    fn reject_older_future_malformed_and_incompatible_versions() {
        for version in ["23", "25", "0", "24junk", "024"] {
            let mut connection = fixture();
            connection
                .execute("UPDATE meta SET value=?1 WHERE key='version'", [version])
                .unwrap();
            assert_eq!(result_kind(&mut connection), Kind::Unsupported);
        }
        let mut connection = fixture();
        connection
            .execute(
                "UPDATE meta SET value='23' WHERE key='last_compatible_version'",
                [],
            )
            .unwrap();
        assert_eq!(result_kind(&mut connection), Kind::Unsupported);
        connection.execute("DELETE FROM meta", []).unwrap();
        assert_eq!(result_kind(&mut connection), Kind::Unsupported);
    }

    #[test]
    fn reject_domain_mixing_duplicates_and_additional_jars() {
        for sql in [
            "UPDATE cookies SET host_key='x.com' WHERE name='ct0'",
            "UPDATE cookies SET name='ct0' WHERE name='auth_token'",
            "INSERT INTO cookies SELECT * FROM cookies",
        ] {
            let mut connection = fixture();
            connection.execute_batch(sql).unwrap();
            assert_eq!(result_kind(&mut connection), Kind::Authentication);
        }
        let mut connection = fixture();
        pair(&connection, "x.com");
        assert_eq!(result_kind(&mut connection), Kind::Authentication);
    }

    #[test]
    fn irrelevant_cookies_are_not_loaded() {
        let mut connection = fixture();
        for host in [".twitter.com", ".evilx.com", ".x.com.evil", "api.x.com"] {
            pair(&connection, host);
        }
        insert(&connection, ".x.com", "unrelated", b"ignored");
        // All irrelevant formats can be unsupported without breaking the pair.
        connection.execute("UPDATE cookies SET encrypted_value=x'763230' WHERE host_key!='.x.com' OR name='unrelated'", []).unwrap();
        assert!(read_transaction(&mut connection, NOW).is_ok());
    }

    #[test]
    fn expiry_security_and_root_path_are_enforced() {
        for sql in [
            "UPDATE cookies SET has_expires=1,is_persistent=1,expires_utc=13000000000000000",
            "UPDATE cookies SET has_expires=1,is_persistent=1,expires_utc=12999999999999999",
            "UPDATE cookies SET expires_utc=-1",
            "UPDATE cookies SET has_expires=1,is_persistent=1,expires_utc='not-a-time'",
            "UPDATE cookies SET has_expires=1,is_persistent=1,expires_utc=1.0e100",
            "UPDATE cookies SET has_expires=1",
            "UPDATE cookies SET is_secure=0",
            "UPDATE cookies SET path='/i/api'",
        ] {
            let mut connection = fixture();
            connection.execute_batch(sql).unwrap();
            assert_eq!(result_kind(&mut connection), Kind::Authentication);
        }
        let mut connection = fixture();
        connection
            .execute_batch(
                "UPDATE cookies SET has_expires=1,is_persistent=1,expires_utc=13000000000000001",
            )
            .unwrap();
        assert!(read_transaction(&mut connection, NOW).is_ok());
    }

    #[test]
    fn reject_partition_plaintext_mixed_unknown_and_oversize_storage() {
        for sql in [
            "UPDATE cookies SET top_frame_site_key='https://x.com'",
            "UPDATE cookies SET has_cross_site_ancestor=2",
            "UPDATE cookies SET source_scheme=1",
            "UPDATE cookies SET source_port=8443",
            "UPDATE cookies SET value='synthetic-plaintext'",
            "UPDATE cookies SET value=char(0)||'hidden'",
            "UPDATE cookies SET encrypted_value=x'76323000000000000000000000000000000000'",
            "UPDATE cookies SET encrypted_value=x''",
            "UPDATE cookies SET encrypted_value=zeroblob(9999)",
        ] {
            let mut connection = fixture();
            connection.execute_batch(sql).unwrap();
            assert_eq!(result_kind(&mut connection), Kind::Unsupported);
        }
        let mut connection = fixture();
        connection
            .execute_batch("UPDATE cookies SET has_cross_site_ancestor=0")
            .unwrap();
        assert!(read_transaction(&mut connection, NOW).is_ok());
    }

    #[test]
    fn readonly_wal_reader_sees_committed_not_uncommitted_rows_and_does_not_write() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Cookies");
        let writer = Connection::open(&path).unwrap();
        writer
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .unwrap();
        schema(&writer);
        pair(&writer, ".x.com");
        let main_before = std::fs::read(&path).unwrap();
        let wal_path = directory.path().join("Cookies-wal");
        let wal_before = std::fs::read(&wal_path).unwrap();
        writer
            .execute_batch("BEGIN; UPDATE cookies SET encrypted_value=x'763230';")
            .unwrap();
        assert!(
            read(&std::fs::canonicalize(&path).unwrap())
                .unwrap()
                .decrypt(PASSWORD)
                .is_ok()
        );
        assert_eq!(std::fs::read(&path).unwrap(), main_before);
        assert_eq!(std::fs::read(&wal_path).unwrap(), wal_before);
        writer.execute_batch("ROLLBACK;").unwrap();
        let reader = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert!(reader.execute("DELETE FROM cookies", []).is_err());
    }

    #[test]
    fn malformed_schema_views_and_partition_conflicts_fail_closed() {
        for sql in [
            "ALTER TABLE cookies DROP COLUMN source_scheme",
            "ALTER TABLE cookies RENAME TO original; CREATE VIEW cookies AS SELECT * FROM original",
            "ALTER TABLE meta RENAME TO original; CREATE VIEW meta AS SELECT * FROM original",
        ] {
            let mut connection = fixture();
            connection.execute_batch(sql).unwrap();
            assert_eq!(result_kind(&mut connection), Kind::Unsupported);
        }
        let mut connection = fixture();
        insert(&connection, ".x.com", "ct0", b"other-synthetic-csrf");
        connection
            .execute(
                "UPDATE cookies SET top_frame_site_key='https://example.com' WHERE rowid=3",
                [],
            )
            .unwrap();
        assert!(read_transaction(&mut connection, NOW).is_err());
    }

    #[test]
    fn locked_database_fails_without_recovery_or_disk_copy() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Cookies");
        let writer = Connection::open(&path).unwrap();
        schema(&writer);
        pair(&writer, ".x.com");
        writer.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let result = read(&std::fs::canonicalize(&path).unwrap());
        assert!(matches!(
            result,
            Err(Error {
                kind: Kind::Storage,
                ..
            })
        ));
        writer.execute_batch("ROLLBACK;").unwrap();
        assert!(read(&std::fs::canonicalize(&path).unwrap()).is_ok());
        let names: Vec<_> = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("Cookies")]);
    }

    #[test]
    fn transaction_keeps_schema_and_cookie_snapshot_together() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Cookies");
        let writer = Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        schema(&writer);
        pair(&writer, ".x.com");
        let mut reader =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let transaction = reader.transaction().unwrap();
        check_schema(&transaction).unwrap();
        writer
            .execute_batch("BEGIN; UPDATE meta SET value='25'; DELETE FROM cookies; COMMIT;")
            .unwrap();
        assert!(
            select_pair(&transaction, NOW)
                .unwrap()
                .decrypt(PASSWORD)
                .is_ok()
        );
        transaction.commit().unwrap();
        assert_eq!(result_kind(&mut reader), Kind::Unsupported);
    }
}
