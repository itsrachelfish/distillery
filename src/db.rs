//! The SQLite database holding everything Distillery records.
//!
//! The schema is a list of steps. SQLite's `user_version` stores how many
//! steps a database has had, so older databases are brought up to date by
//! running the missing steps. Each build step of the MVP adds the schema step
//! it needs, so no table exists before the code that uses it.

use std::error::Error;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, params};

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// How long a write waits for another one to finish before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The schema, as the steps that build it. Steps are only ever added at the
/// end: released steps never change.
const MIGRATIONS: [&str; 1] = [SCHEMA_V1];

const SCHEMA_V1: &str = "
-- Git repositories added by users.
CREATE TABLE sources (
    id INTEGER PRIMARY KEY,
    -- The URL as first entered.
    url TEXT NOT NULL,
    -- The clone's path under the clone folder, such as
    -- github.com/wetfish/server. Different URLs of one repository share it.
    clone_folder TEXT NOT NULL UNIQUE,
    -- UTC, as YYYY-MM-DD HH:MM:SS.
    added_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Commits that snapshots found a source at.
CREATE TABLE revisions (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL REFERENCES sources (id),
    -- The full commit hash.
    commit_hash TEXT NOT NULL,
    -- UTC, as YYYY-MM-DD HH:MM:SS, when a snapshot first found it.
    recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (source_id, commit_hash)
);
";

/// Opens the database, creating it when the file is new and running the
/// schema steps an older database is missing.
pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&conn, &MIGRATIONS)?;
    // Write-ahead logging, so pages keep reading while a run writes. It
    // stays set in the file. Writers wait for each other a while rather than
    // failing at once.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(conn)
}

/// How many schema steps the database has had.
pub fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

/// The source cloned into `clone_folder`, added when it is new.
pub fn source(conn: &Connection, url: &str, clone_folder: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO sources (url, clone_folder) VALUES (?1, ?2)
         ON CONFLICT (clone_folder) DO NOTHING",
        params![url, clone_folder],
    )?;
    Ok(conn.query_row(
        "SELECT id FROM sources WHERE clone_folder = ?1",
        params![clone_folder],
        |row| row.get(0),
    )?)
}

/// Records that a source was found at a commit. Returns whether the commit
/// is new for the source.
pub fn record_revision(conn: &Connection, source_id: i64, commit_hash: &str) -> Result<bool> {
    let added = conn.execute(
        "INSERT INTO revisions (source_id, commit_hash) VALUES (?1, ?2)
         ON CONFLICT (source_id, commit_hash) DO NOTHING",
        params![source_id, commit_hash],
    )?;
    Ok(added > 0)
}

/// Runs the steps the database is missing, each in a transaction that also
/// records it, so a failed step leaves the database as it was.
fn migrate(conn: &Connection, steps: &[&str]) -> Result<()> {
    let version = schema_version(conn)?;
    let latest = steps.len() as i64;
    if !(0..=latest).contains(&version) {
        return Err(format!(
            "it uses database schema version {version}, but this version of distillery only \
             knows up to {latest}. It holds reviewed decisions, so use a newer distillery \
             rather than deleting it, or set a different storage.database"
        )
        .into());
    }
    for (step, sql) in steps.iter().enumerate().skip(version as usize) {
        let version = step + 1;
        let batch = format!("BEGIN; {sql} PRAGMA user_version = {version}; COMMIT;");
        if let Err(err) = conn.execute_batch(&batch) {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(format!("schema step {version} failed: {err}").into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    const STEP_1: &str = "CREATE TABLE first (id INTEGER PRIMARY KEY);";
    const STEP_2: &str = "CREATE TABLE second (
        id INTEGER PRIMARY KEY,
        first_id INTEGER NOT NULL REFERENCES first (id)
    );";

    fn tables(conn: &Connection) -> Vec<String> {
        let mut statement = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn creates_the_database_file() {
        let root = TempDir::new("db-create");
        let path = root.0.join("distillery.sqlite");
        let conn = open(&path).unwrap();
        assert!(path.exists());
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn opens_an_existing_database_again() {
        let root = TempDir::new("db-reopen");
        let path = root.0.join("distillery.sqlite");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn adds_sources_once_per_clone_folder() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, &MIGRATIONS).unwrap();
        let folder = "github.com/wetfish/server";
        let first = source(&conn, "https://github.com/wetfish/server", folder).unwrap();
        let again = source(&conn, "git@github.com:wetfish/server.git", folder).unwrap();
        assert_eq!(first, again);
        let url: String = conn
            .query_row("SELECT url FROM sources", [], |row| row.get(0))
            .unwrap();
        assert_eq!(url, "https://github.com/wetfish/server");

        let other = source(
            &conn,
            "https://github.com/wetfish/wiki",
            "github.com/wetfish/wiki",
        );
        assert_ne!(other.unwrap(), first);
    }

    #[test]
    fn records_revisions_once_per_commit() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&conn, &MIGRATIONS).unwrap();
        let id = source(&conn, "https://example.net/a/b", "example.net/a/b").unwrap();
        assert!(record_revision(&conn, id, "abc123").unwrap());
        assert!(!record_revision(&conn, id, "abc123").unwrap());
        assert!(record_revision(&conn, id, "def456").unwrap());
        // Revisions belong to a source.
        assert!(record_revision(&conn, id + 1, "abc123").is_err());
    }

    #[test]
    fn runs_every_step_on_a_new_database() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, &[STEP_1, STEP_2]).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 2);
        assert_eq!(tables(&conn), ["first", "second"]);
    }

    #[test]
    fn runs_only_the_missing_steps() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, &[STEP_1]).unwrap();
        conn.execute("INSERT INTO first (id) VALUES (7)", [])
            .unwrap();

        // Running step 1 again would fail, since its table exists.
        migrate(&conn, &[STEP_1, STEP_2]).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 2);
        let kept: i64 = conn
            .query_row("SELECT id FROM first", [], |row| row.get(0))
            .unwrap();
        assert_eq!(kept, 7);

        // Nothing is missing now.
        migrate(&conn, &[STEP_1, STEP_2]).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 2);
    }

    #[test]
    fn failed_steps_change_nothing() {
        let conn = Connection::open_in_memory().unwrap();
        let broken = "CREATE TABLE second (id INTEGER PRIMARY KEY); CREATE TABLE first (id);";
        let err = migrate(&conn, &[STEP_1, broken]).unwrap_err();
        assert!(err.to_string().contains("schema step 2"), "{err}");
        assert_eq!(schema_version(&conn).unwrap(), 1);
        assert_eq!(tables(&conn), ["first"]);
    }

    #[test]
    fn refuses_a_database_from_a_newer_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA user_version = 99;").unwrap();
        let err = migrate(&conn, &[STEP_1]).unwrap_err();
        assert!(err.to_string().contains("version 99"), "{err}");
        assert!(tables(&conn).is_empty());
    }
}
