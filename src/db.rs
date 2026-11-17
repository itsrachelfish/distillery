//! The SQLite database holding everything Distillery records.
//!
//! The schema is a list of steps. SQLite's `user_version` stores how many
//! steps a database has had, so older databases are brought up to date by
//! running the missing steps. Each build step of the MVP adds the schema step
//! it needs, so no table exists before the code that uses it.

use std::error::Error;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// How long a write waits for another one to finish before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The schema, as the steps that build it. Steps are only ever added at the
/// end: released steps never change.
const MIGRATIONS: [&str; 0] = [];

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
pub mod testing {
    //! Temporary folders for tests that need files.

    use std::fs;
    use std::path::PathBuf;

    /// A folder under the system's temporary folder, removed when dropped.
    pub struct TempDir(pub PathBuf);

    impl TempDir {
        /// `name` must be unique among tests, which run in parallel.
        pub fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("distillery-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::TempDir;
    use super::*;

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
