//! The SQLite database holding everything Distillery records.
//!
//! The schema is a list of steps. SQLite's `user_version` stores how many
//! steps a database has had, so older databases are brought up to date by
//! running the missing steps. Each build step of the MVP adds the schema step
//! it needs, so no table exists before the code that uses it.

use std::error::Error;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// How long a write waits for another one to finish before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The schema, as the steps that build it. Steps are only ever added at the
/// end: released steps never change.
const MIGRATIONS: [&str; 3] = [SCHEMA_V1, SCHEMA_V2, SCHEMA_V3];

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

const SCHEMA_V2: &str = "
-- Every call to a model API, failed calls included, for usage and cost.
CREATE TABLE model_calls (
    id INTEGER PRIMARY KEY,
    -- The API called, such as anthropic.
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    -- What the call was for, such as check_key.
    operation TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    -- Input tokens written to and read from the provider's prompt cache.
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    -- Millionths of a US dollar, from the prices configured at call time.
    cost_micros INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL,
    -- Why the model stopped, when the reply says.
    stop_reason TEXT,
    -- 1 when the call returned a usable reply.
    success INTEGER NOT NULL,
    error TEXT,
    -- UTC, as YYYY-MM-DD HH:MM:SS.
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX model_calls_created_at ON model_calls (created_at);
";

const SCHEMA_V3: &str = "
-- The HTTP status of a failed call's reply, such as 429 for a rate limit.
-- NULL for successful calls and for calls that got no reply.
ALTER TABLE model_calls ADD COLUMN http_status INTEGER;

-- What distilling a file was expected to cost, counted before any run and
-- never changed, for comparison with what runs cost.
CREATE TABLE estimates (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL REFERENCES sources (id),
    -- The full commit hash the file was read at.
    commit_hash TEXT NOT NULL,
    path TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    -- The hash of the prompt and reply schema that were counted.
    prompt_version TEXT NOT NULL,
    -- Sections holding a heading alone, which need no call.
    skipped_sections INTEGER NOT NULL,
    -- Input tokens over every section to send, as the provider counted them.
    input_tokens INTEGER NOT NULL,
    -- Millionths of a US dollar, from the prices configured at the time.
    input_cost_micros INTEGER NOT NULL,
    -- Expected from the output-to-input ratio of the model's earlier
    -- extraction calls. NULL when there were none.
    output_tokens INTEGER,
    output_cost_micros INTEGER,
    -- How many earlier calls the ratio came from.
    ratio_calls INTEGER NOT NULL,
    -- UTC, as YYYY-MM-DD HH:MM:SS.
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX estimates_file ON estimates (source_id, commit_hash, path);

-- The sections an estimate counted, one call each.
CREATE TABLE estimate_sections (
    estimate_id INTEGER NOT NULL REFERENCES estimates (id),
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    -- The system prompt, reply schema and section together, as one call
    -- sends them.
    input_tokens INTEGER NOT NULL,
    PRIMARY KEY (estimate_id, start_line)
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

/// A source with its latest recorded commit, for the Sources page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSummary {
    pub id: i64,
    pub url: String,
    /// The clone's path under the clone folder, which names the source.
    pub clone_folder: String,
    /// The commit of the latest snapshot, with when it was recorded in UTC.
    pub latest: Option<(String, String)>,
    /// How many commits snapshots have recorded.
    pub revisions: i64,
}

/// Every source, by name.
pub fn sources(conn: &Connection) -> Result<Vec<SourceSummary>> {
    let mut statement = conn.prepare(
        "SELECT s.id, s.url, s.clone_folder, latest.commit_hash, latest.recorded_at,
                (SELECT count(*) FROM revisions r WHERE r.source_id = s.id)
         FROM sources s
         LEFT JOIN revisions latest ON latest.id =
             (SELECT max(r.id) FROM revisions r WHERE r.source_id = s.id)
         ORDER BY s.clone_folder",
    )?;
    let rows = statement.query_map([], |row| {
        let commit: Option<String> = row.get(3)?;
        let recorded_at: Option<String> = row.get(4)?;
        Ok(SourceSummary {
            id: row.get(0)?,
            url: row.get(1)?,
            clone_folder: row.get(2)?,
            latest: commit.zip(recorded_at),
            revisions: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A git repository added by a user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: i64,
    pub url: String,
    /// The clone's path under the clone folder, which names the source.
    pub clone_folder: String,
}

/// The source with this ID, if there is one.
pub fn find_source(conn: &Connection, id: i64) -> Result<Option<Source>> {
    Ok(conn
        .query_row(
            "SELECT id, url, clone_folder FROM sources WHERE id = ?1",
            params![id],
            |row| {
                Ok(Source {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    clone_folder: row.get(2)?,
                })
            },
        )
        .optional()?)
}

/// A commit that a snapshot found a source at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    /// The full commit hash.
    pub commit_hash: String,
    /// UTC, as YYYY-MM-DD HH:MM:SS.
    pub recorded_at: String,
}

/// The commits recorded for a source, latest first.
pub fn revisions(conn: &Connection, source_id: i64) -> Result<Vec<Revision>> {
    let mut statement = conn.prepare(
        "SELECT commit_hash, recorded_at FROM revisions WHERE source_id = ?1 ORDER BY id DESC",
    )?;
    let rows = statement.query_map(params![source_id], |row| {
        Ok(Revision {
            commit_hash: row.get(0)?,
            recorded_at: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// One call to a model API, as recorded for usage and cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCall {
    pub provider: String,
    pub model: String,
    pub operation: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_write_tokens: i64,
    pub cache_read_tokens: i64,
    /// Millionths of a US dollar.
    pub cost_micros: i64,
    pub duration_ms: i64,
    pub stop_reason: Option<String>,
    /// `None` when the call returned a usable reply.
    pub error: Option<String>,
    /// The HTTP status of a failed call's reply, when one arrived.
    pub http_status: Option<i64>,
}

/// Records a call to a model API.
pub fn record_model_call(conn: &Connection, call: &ModelCall) -> Result<()> {
    conn.execute(
        "INSERT INTO model_calls (provider, model, operation, input_tokens, output_tokens,
             cache_write_tokens, cache_read_tokens, cost_micros, duration_ms, stop_reason,
             success, error, http_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            call.provider,
            call.model,
            call.operation,
            call.input_tokens,
            call.output_tokens,
            call.cache_write_tokens,
            call.cache_read_tokens,
            call.cost_micros,
            call.duration_ms,
            call.stop_reason,
            call.error.is_none(),
            call.error,
            call.http_status,
        ],
    )?;
    Ok(())
}

/// Calls added up over a group, such as one model or one day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageTotal {
    /// What the group is: a model name, a day, or an operation.
    pub group: String,
    pub calls: i64,
    pub failed: i64,
    /// Failed calls the API turned away with a rate limit.
    pub rate_limited: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_micros: i64,
}

/// How usage totals are grouped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageGroup {
    Model,
    Operation,
    /// UTC days, latest first.
    Day,
}

/// Usage added up by model, operation or day.
pub fn usage_totals(conn: &Connection, group: UsageGroup) -> Result<Vec<UsageTotal>> {
    // Both parts are fixed text, never user input.
    let (column, order) = match group {
        UsageGroup::Model => ("model", "1"),
        UsageGroup::Operation => ("operation", "1"),
        UsageGroup::Day => ("date(created_at)", "1 DESC"),
    };
    let mut statement = conn.prepare(&format!(
        "SELECT {column}, count(*), sum(1 - success),
                count(CASE WHEN http_status = 429 THEN 1 END),
                sum(input_tokens), sum(output_tokens), sum(cost_micros)
         FROM model_calls GROUP BY 1 ORDER BY {order}"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok(UsageTotal {
            group: row.get(0)?,
            calls: row.get(1)?,
            failed: row.get(2)?,
            rate_limited: row.get(3)?,
            input_tokens: row.get(4)?,
            output_tokens: row.get(5)?,
            cost_micros: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A recorded call with when it was made, in UTC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    pub created_at: String,
    pub call: ModelCall,
}

/// The latest calls, newest first.
pub fn latest_model_calls(conn: &Connection, limit: i64) -> Result<Vec<RecordedCall>> {
    let mut statement = conn.prepare(
        "SELECT created_at, provider, model, operation, input_tokens, output_tokens,
                cache_write_tokens, cache_read_tokens, cost_micros, duration_ms, stop_reason,
                error, http_status
         FROM model_calls ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = statement.query_map(params![limit], |row| {
        Ok(RecordedCall {
            created_at: row.get(0)?,
            call: ModelCall {
                provider: row.get(1)?,
                model: row.get(2)?,
                operation: row.get(3)?,
                input_tokens: row.get(4)?,
                output_tokens: row.get(5)?,
                cache_write_tokens: row.get(6)?,
                cache_read_tokens: row.get(7)?,
                cost_micros: row.get(8)?,
                duration_ms: row.get(9)?,
                stop_reason: row.get(10)?,
                error: row.get(11)?,
                http_status: row.get(12)?,
            },
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Output and input tokens of earlier calls, for expecting output before a
/// run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputRatio {
    pub calls: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

impl OutputRatio {
    /// The output tokens to expect for `input_tokens` of input. `None`
    /// without earlier calls to go by.
    pub fn output_for(&self, input_tokens: i64) -> Option<i64> {
        if self.calls == 0 || self.input_tokens == 0 {
            return None;
        }
        let ratio = self.output_tokens as f64 / self.input_tokens as f64;
        Some((input_tokens as f64 * ratio).round() as i64)
    }
}

/// Tokens added up over a model's successful calls for one operation.
pub fn output_ratio(conn: &Connection, operation: &str, model: &str) -> Result<OutputRatio> {
    Ok(conn.query_row(
        "SELECT count(*), coalesce(sum(input_tokens), 0), coalesce(sum(output_tokens), 0)
         FROM model_calls WHERE operation = ?1 AND model = ?2 AND success = 1",
        params![operation, model],
        |row| {
            Ok(OutputRatio {
                calls: row.get(0)?,
                input_tokens: row.get(1)?,
                output_tokens: row.get(2)?,
            })
        },
    )?)
}

/// What distilling a file was expected to cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimate {
    pub source_id: i64,
    /// The full commit hash the file was read at.
    pub commit_hash: String,
    pub path: String,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    /// Sections holding a heading alone, which need no call.
    pub skipped_sections: i64,
    pub input_tokens: i64,
    /// Millionths of a US dollar.
    pub input_cost_micros: i64,
    /// `None` without earlier extraction calls to go by.
    pub output_tokens: Option<i64>,
    pub output_cost_micros: Option<i64>,
    /// How many earlier calls the expected output came from.
    pub ratio_calls: i64,
}

/// A section an estimate counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstimateSection {
    pub start_line: i64,
    pub end_line: i64,
    pub input_tokens: i64,
}

/// Records an estimate with its sections, and returns its ID.
pub fn record_estimate(
    conn: &mut Connection,
    estimate: &Estimate,
    sections: &[EstimateSection],
) -> Result<i64> {
    let transaction = conn.transaction()?;
    transaction.execute(
        "INSERT INTO estimates (source_id, commit_hash, path, provider, model, prompt_version,
             skipped_sections, input_tokens, input_cost_micros, output_tokens,
             output_cost_micros, ratio_calls)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            estimate.source_id,
            estimate.commit_hash,
            estimate.path,
            estimate.provider,
            estimate.model,
            estimate.prompt_version,
            estimate.skipped_sections,
            estimate.input_tokens,
            estimate.input_cost_micros,
            estimate.output_tokens,
            estimate.output_cost_micros,
            estimate.ratio_calls,
        ],
    )?;
    let id = transaction.last_insert_rowid();
    for section in sections {
        transaction.execute(
            "INSERT INTO estimate_sections (estimate_id, start_line, end_line, input_tokens)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                id,
                section.start_line,
                section.end_line,
                section.input_tokens
            ],
        )?;
    }
    transaction.commit()?;
    Ok(id)
}

/// An estimate with its ID and when it was counted, in UTC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedEstimate {
    pub id: i64,
    pub created_at: String,
    pub estimate: Estimate,
}

const ESTIMATE_COLUMNS: &str = "id, created_at, source_id, commit_hash, path, provider, model,
    prompt_version, skipped_sections, input_tokens, input_cost_micros, output_tokens,
    output_cost_micros, ratio_calls";

fn read_estimate(row: &rusqlite::Row) -> rusqlite::Result<RecordedEstimate> {
    Ok(RecordedEstimate {
        id: row.get(0)?,
        created_at: row.get(1)?,
        estimate: Estimate {
            source_id: row.get(2)?,
            commit_hash: row.get(3)?,
            path: row.get(4)?,
            provider: row.get(5)?,
            model: row.get(6)?,
            prompt_version: row.get(7)?,
            skipped_sections: row.get(8)?,
            input_tokens: row.get(9)?,
            input_cost_micros: row.get(10)?,
            output_tokens: row.get(11)?,
            output_cost_micros: row.get(12)?,
            ratio_calls: row.get(13)?,
        },
    })
}

/// The estimate with this ID, if there is one.
pub fn find_estimate(conn: &Connection, id: i64) -> Result<Option<RecordedEstimate>> {
    Ok(conn
        .query_row(
            &format!("SELECT {ESTIMATE_COLUMNS} FROM estimates WHERE id = ?1"),
            params![id],
            read_estimate,
        )
        .optional()?)
}

/// The latest estimate for a file at a commit, if it has one.
pub fn latest_estimate(
    conn: &Connection,
    source_id: i64,
    commit_hash: &str,
    path: &str,
) -> Result<Option<RecordedEstimate>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {ESTIMATE_COLUMNS} FROM estimates
                 WHERE source_id = ?1 AND commit_hash = ?2 AND path = ?3
                 ORDER BY id DESC LIMIT 1"
            ),
            params![source_id, commit_hash, path],
            read_estimate,
        )
        .optional()?)
}

/// The sections an estimate counted, in file order.
pub fn estimate_sections(conn: &Connection, estimate_id: i64) -> Result<Vec<EstimateSection>> {
    let mut statement = conn.prepare(
        "SELECT start_line, end_line, input_tokens FROM estimate_sections
         WHERE estimate_id = ?1 ORDER BY start_line",
    )?;
    let rows = statement.query_map(params![estimate_id], |row| {
        Ok(EstimateSection {
            start_line: row.get(0)?,
            end_line: row.get(1)?,
            input_tokens: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
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
