//! The snapshot stage: clones a repository, or fetches a known one, records
//! the commit its default branch is at, and lists its Markdown files at that
//! commit.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::db;
use crate::git::{self, RepoUrl};
use crate::progress::{Progress, counted};

/// What a snapshot found.
#[derive(Debug)]
pub struct Snapshot {
    /// Where the bare clone lives.
    pub clone: PathBuf,
    /// The full commit hash of the default branch.
    pub commit: String,
    /// Whether no earlier snapshot recorded this commit.
    pub new_commit: bool,
    /// Markdown files at the commit, sorted by path.
    pub files: Vec<String>,
}

/// Snapshots the repository at a URL entered by a user.
pub fn run(
    conn: &Connection,
    clone_dir: &Path,
    url: &str,
    progress: &mut Progress,
) -> db::Result<Snapshot> {
    let repo = git::parse_url(url)?;
    snapshot(conn, clone_dir, &repo, progress)
}

fn snapshot(
    conn: &Connection,
    clone_dir: &Path,
    repo: &RepoUrl,
    progress: &mut Progress,
) -> db::Result<Snapshot> {
    let clone = clone_dir.join(&repo.folder);
    // Clones and fetches show git's own progress lines.
    if git::is_cloned(&clone) {
        progress.note(&format!("Fetching into {}", clone.display()));
        progress.stage("Fetching", 0, "");
        git::fetch(&clone, &mut |line| progress.status(line))?;
        progress.done("up to date with the remote");
    } else {
        progress.note(&format!("Cloning {} into {}", repo.url, clone.display()));
        progress.stage("Cloning", 0, "");
        git::clone(&repo.url, &clone, &mut |line| progress.status(line))?;
        progress.done("cloned with full history");
    }
    let commit = git::head(&clone)?;

    // Forward slashes on every system, so databases move between machines.
    let folder = repo.folder.to_string_lossy().replace('\\', "/");
    let source_id = db::source(conn, &repo.url, &folder)?;
    let new_commit = db::record_revision(conn, source_id, &commit)?;

    progress.stage("Listing Markdown files", 0, "tracked files");
    let files = git::markdown_files(&clone, &commit, |seen, path| {
        progress.update(seen, || path.to_owned());
    })?;
    progress.done(&counted(
        files.len() as u64,
        "Markdown file",
        "Markdown files",
    ));

    Ok(Snapshot {
        clone,
        commit,
        new_commit,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{self, TempDir};

    fn revisions(conn: &Connection) -> Vec<String> {
        let mut statement = conn
            .prepare("SELECT commit_hash FROM revisions ORDER BY id")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn clones_then_fetches_and_records_each_commit_once() {
        let root = TempDir::new("snapshot");
        let origin = root.0.join("origin");
        let first = testing::repo(
            &origin,
            &[("README.md", "# Readme\n"), ("src/lib.rs", "// code\n")],
        );
        let conn = db::open(&root.0.join("distillery.sqlite")).unwrap();
        let clone_dir = root.0.join("repos");
        // Local paths are not accepted from users, so the test skips the URL
        // check.
        let repo = RepoUrl {
            url: origin.to_string_lossy().into_owned(),
            folder: PathBuf::from("local/origin"),
        };
        let mut progress = Progress::quiet();

        let found = snapshot(&conn, &clone_dir, &repo, &mut progress).unwrap();
        assert_eq!(found.clone, clone_dir.join("local/origin"));
        assert_eq!(found.commit, first);
        assert!(found.new_commit);
        assert_eq!(found.files, ["README.md"]);

        // Nothing changed: the same commit, recorded once.
        let found = snapshot(&conn, &clone_dir, &repo, &mut progress).unwrap();
        assert_eq!(found.commit, first);
        assert!(!found.new_commit);

        // A new commit is fetched into the same clone.
        let second = testing::commit(&origin, &[("docs/guide.md", "# Guide\n")]);
        let found = snapshot(&conn, &clone_dir, &repo, &mut progress).unwrap();
        assert_eq!(found.commit, second);
        assert!(found.new_commit);
        assert_eq!(found.files, ["README.md", "docs/guide.md"]);

        assert_eq!(revisions(&conn), [first, second]);
        let sources: i64 = conn
            .query_row("SELECT count(*) FROM sources", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sources, 1);
    }

    #[test]
    fn rejects_urls_before_running_git() {
        let root = TempDir::new("snapshot-url");
        let conn = db::open(&root.0.join("distillery.sqlite")).unwrap();
        let clone_dir = root.0.join("repos");
        let mut progress = Progress::quiet();
        let err = run(&conn, &clone_dir, "file:///tmp/repo", &mut progress).unwrap_err();
        assert!(err.to_string().contains("https or ssh"), "{err}");
        assert!(!clone_dir.exists());
    }
}
