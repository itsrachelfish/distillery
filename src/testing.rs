//! Helpers shared by tests: temporary folders and small git repositories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A folder under the system's temporary folder, removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    /// `name` must be unique among tests, which run in parallel.
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("distillery-{name}-{}", std::process::id()));
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

/// Runs git in `dir`, with a fixed author and no settings from the machine.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.net",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("tests need git installed");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Creates a repository in `dir` with one commit holding `files`, as paths
/// and contents, and returns the commit hash.
pub fn repo(dir: &Path, files: &[(&str, &str)]) -> String {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "--quiet", "--initial-branch=main"]);
    commit(dir, files)
}

/// Adds a commit holding `files` and returns its hash.
pub fn commit(dir: &Path, files: &[(&str, &str)]) -> String {
    for (path, contents) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "--message", "test: add files"]);
    git(dir, &["rev-parse", "HEAD"])
}
