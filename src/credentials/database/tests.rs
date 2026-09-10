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
    let mut reader = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
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
