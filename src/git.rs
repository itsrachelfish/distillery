//! Repositories, through native git commands.
//!
//! Clones are bare: no working tree exists, so nothing drifts from the
//! recorded commit and no checkout filters or hooks run. Files are read from
//! git at an exact commit. Clones keep full history, which conflict review
//! needs for the age of each file.

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A repository URL, and where its clone lives under the clone folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoUrl {
    pub url: String,
    /// The host and path, such as `github.com/wetfish/server`.
    pub folder: PathBuf,
}

/// Checks a URL entered by a user. Only https and ssh are accepted, so git
/// never runs local transports or helper programs for it.
pub fn parse_url(input: &str) -> Result<RepoUrl, String> {
    let url = input.trim();
    if url.is_empty() {
        return Err("the repository URL is empty".to_owned());
    }
    if url.starts_with('-') || url.contains(|c: char| c.is_whitespace() || c.is_control()) {
        return Err(format!(
            "{url:?} is not a repository URL: no spaces, and no leading dash"
        ));
    }

    let (host, path) = if let Some(rest) = url.strip_prefix("https://") {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if host.contains('@') {
            return Err(
                "https URLs with a user name or token are not accepted, since the URL is \
                 stored. Use an ssh URL for private repositories"
                    .to_owned(),
            );
        }
        (host, path)
    } else if let Some(rest) = url.strip_prefix("ssh://") {
        rest.split_once('/').unwrap_or((rest, ""))
    } else if !url.contains("://")
        && let Some((host, path)) = url.split_once(':')
        && !host.contains('/')
    {
        // The short ssh form, such as git@github.com:wetfish/server.git
        (host, path)
    } else {
        return Err(format!(
            "{url:?} must be an https or ssh URL, such as https://github.com/wetfish/server \
             or git@github.com:wetfish/server.git"
        ));
    };

    // The folder leaves out the user name and the port.
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = host.split_once(':').map_or(host, |(host, _)| host);
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        return Err(format!("{url:?} needs a host and a repository path"));
    }

    let mut folder = PathBuf::new();
    for part in std::iter::once(host).chain(path.split('/')) {
        let plain = part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
        if part.is_empty() || part == "." || part == ".." || !plain {
            return Err(format!(
                "{url:?} has the part {part:?}, which must be letters, digits, dashes, \
                 underscores and dots"
            ));
        }
        folder.push(part.to_ascii_lowercase());
    }

    Ok(RepoUrl {
        url: url.to_owned(),
        folder,
    })
}

/// Whether `clone` holds a repository already.
pub fn is_cloned(clone: &Path) -> bool {
    clone.join("HEAD").is_file()
}

/// Clones `url` into `clone` as a bare repository. `on_line` gets each line
/// of git's own progress, as git wrote it.
pub fn clone(url: &str, clone: &Path, on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
    if let Some(parent) = clone.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("creating {}: {err}", parent.display()))?;
    }
    let mut command = git();
    command.args(["clone", "--bare", "--progress"]);
    command.arg("--").arg(url).arg(clone);
    run_reported(command, "git clone", on_line)
}

/// Brings every branch of a bare clone up to date with its remote.
/// `on_line` gets each line of git's own progress.
pub fn fetch(clone: &Path, on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
    let mut command = git();
    command.arg("--git-dir").arg(clone);
    command.args(["fetch", "--prune", "--progress"]);
    command.args(["origin", "+refs/heads/*:refs/heads/*"]);
    run_reported(command, "git fetch", on_line)
}

/// The commit hash the clone's default branch points to.
pub fn head(clone: &Path) -> Result<String, String> {
    let mut command = git();
    command.arg("--git-dir").arg(clone);
    command.args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]);
    let hash = run_captured(command, "git rev-parse")
        .map_err(|_| "the repository has no commits on its default branch".to_owned())?;
    Ok(String::from_utf8_lossy(&hash).trim().to_owned())
}

/// The Markdown files at `commit`, sorted by path. `seen` is told how many
/// tracked files have been looked at so far, and the path of the last one.
pub fn markdown_files(
    clone: &Path,
    commit: &str,
    mut seen: impl FnMut(u64, &str),
) -> Result<Vec<String>, String> {
    let mut command = git();
    command.arg("--git-dir").arg(clone);
    // One entry per tracked file: "<mode> <type> <hash>\t<path>", ending
    // with a zero byte so unusual paths arrive unquoted.
    command.args(["ls-tree", "-r", "-z", "--full-tree"]);
    command.arg(commit);
    let listing = run_captured(command, "git ls-tree")?;

    let mut files = Vec::new();
    let mut count = 0;
    for entry in listing.split(|&byte| byte == 0) {
        if entry.is_empty() {
            continue;
        }
        let entry = String::from_utf8_lossy(entry);
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        count += 1;
        seen(count, path);
        let mut meta = meta.split(' ');
        let (mode, kind) = (meta.next(), meta.next());
        // Symbolic links (mode 120000) and submodules (type commit) hold no
        // Markdown of their own.
        if kind == Some("blob") && mode != Some("120000") && is_markdown(path) {
            files.push(path.to_owned());
        }
    }
    files.sort();
    Ok(files)
}

/// The contents of a file at `commit`. Bytes that are not UTF-8 show as
/// replacement characters.
pub fn file_at(clone: &Path, commit: &str, path: &str) -> Result<String, String> {
    let mut command = git();
    command.arg("--git-dir").arg(clone);
    command.args(["cat-file", "blob"]);
    command.arg(format!("{commit}:{path}"));
    let contents = run_captured(command, "git cat-file")?;
    Ok(String::from_utf8_lossy(&contents).into_owned())
}

/// The hash git gives a file with these contents, as `git hash-object`
/// prints it. Nothing is written to any repository.
pub fn hash_object(contents: &[u8]) -> Result<String, String> {
    let name = "git hash-object";
    let mut child = git()
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("running {name}: {err}. Is git installed?"))?;
    // Git reads everything before it prints the hash, and the input is
    // closed when `stdin` is dropped at the end of this statement.
    let written = child
        .stdin
        .take()
        .expect("standard input is piped")
        .write_all(contents);
    let output = child
        .wait_with_output()
        .map_err(|err| format!("waiting for {name}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    written.map_err(|err| format!("writing to {name}: {err}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Whether a path ends in `.md` or `.markdown`, in any letter case.
fn is_markdown(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && (extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown"))
    })
}

/// A git command that never waits for a password or other input.
fn git() -> Command {
    let mut command = Command::new("git");
    command.env("GIT_TERMINAL_PROMPT", "0").stdin(Stdio::null());
    command
}

/// How many of git's last messages an error quotes.
const ERROR_LINES: usize = 3;

/// Runs a command that reports progress on standard error, passing each
/// line to `on_line`. Git redraws a progress line by ending it with a
/// carriage return, so both line endings count.
fn run_reported(
    mut command: Command,
    name: &str,
    on_line: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("running {name}: {err}. Is git installed?"))?;
    let mut stderr = child.stderr.take().expect("standard error is piped");

    // Lines that stayed on screen, which is where git's errors are.
    let mut messages: Vec<String> = Vec::new();
    let mut line = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let read = match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        for &byte in &buffer[..read] {
            if byte != b'\r' && byte != b'\n' {
                line.push(byte);
                continue;
            }
            let text = String::from_utf8_lossy(&line).trim().to_owned();
            line.clear();
            if text.is_empty() {
                continue;
            }
            on_line(&text);
            if byte == b'\n' {
                messages.push(text);
            }
        }
    }
    let last = String::from_utf8_lossy(&line).trim().to_owned();
    if !last.is_empty() {
        on_line(&last);
        messages.push(last);
    }

    let status = child
        .wait()
        .map_err(|err| format!("waiting for {name}: {err}"))?;
    if status.success() {
        return Ok(());
    }
    let from = messages.len().saturating_sub(ERROR_LINES);
    let said = messages[from..].join(" ");
    if said.is_empty() {
        Err(format!("{name} failed without a message"))
    } else {
        Err(format!("{name} failed: {said}"))
    }
}

/// Runs a command and returns what it printed.
fn run_captured(mut command: Command, name: &str) -> Result<Vec<u8>, String> {
    let output = command
        .output()
        .map_err(|err| format!("running {name}: {err}. Is git installed?"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{name} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{self, TempDir};

    fn folder(url: &str) -> String {
        parse_url(url)
            .unwrap_or_else(|err| panic!("{url}: {err}"))
            .folder
            .to_string_lossy()
            .replace('\\', "/")
    }

    #[test]
    fn names_clone_folders_after_the_url() {
        for url in [
            "https://github.com/wetfish/server",
            "https://github.com/wetfish/server.git",
            "https://GitHub.com/wetfish/server/",
            "  https://github.com/wetfish/server\n",
            "ssh://git@github.com/wetfish/server.git",
            "ssh://git@github.com:22/wetfish/server.git",
            "git@github.com:wetfish/server.git",
        ] {
            assert_eq!(folder(url), "github.com/wetfish/server", "{url}");
        }
        assert_eq!(
            folder("https://gitlab.example.net/group/sub.group/my_repo-2"),
            "gitlab.example.net/group/sub.group/my_repo-2"
        );
    }

    #[test]
    fn keeps_the_url_as_entered() {
        let repo = parse_url(" git@github.com:wetfish/server.git ").unwrap();
        assert_eq!(repo.url, "git@github.com:wetfish/server.git");
    }

    #[test]
    fn rejects_urls_git_should_never_see() {
        for url in [
            "",
            "   ",
            "--upload-pack=touch /tmp/x",
            "-oProxyCommand=x",
            "http://github.com/wetfish/server",
            "git://github.com/wetfish/server",
            "file:///home/rachel/repo",
            "ext::sh -c touch% /tmp/x",
            "ext::sh",
            "/home/rachel/repo",
            "../repo",
            "repo",
            "https://github.com",
            "https://github.com/",
            "https://token@github.com/wetfish/server",
            "https://github.com/wetfish/../../etc",
            "https://github.com/wetfish//server",
            "https://github.com/wetfish/server?x=1",
            "git@github.com:../server",
            "https://github.com/wet fish/server",
        ] {
            assert!(parse_url(url).is_err(), "accepted {url:?}");
        }
    }

    #[test]
    fn recognizes_markdown_files() {
        for path in ["README.md", "docs/a.MD", "docs/deep/b.Markdown", "a.b.md"] {
            assert!(is_markdown(path), "{path}");
        }
        for path in [
            "README",
            "notes.txt",
            "md",
            ".md",
            "docs/.md",
            "a.mdx",
            "x.md/y",
        ] {
            assert!(!is_markdown(path), "{path}");
        }
    }

    #[test]
    fn clones_bare_and_lists_markdown_at_the_commit() {
        let root = TempDir::new("git-clone");
        let origin = root.0.join("origin");
        let first = testing::repo(
            &origin,
            &[
                ("README.md", "# Readme\n"),
                ("docs/b.md", "# B\n"),
                ("docs/a.MD", "# A\n"),
                ("docs/odd name ü.md", "# Odd\n"),
                ("src/main.rs", "fn main() {}\n"),
            ],
        );

        let clone_path = root.0.join("clones/host/repo");
        assert!(!is_cloned(&clone_path));
        clone(origin.to_str().unwrap(), &clone_path, &mut |_| ()).unwrap();
        assert!(is_cloned(&clone_path));
        assert!(!clone_path.join("README.md").exists(), "clones are bare");
        assert_eq!(head(&clone_path).unwrap(), first);

        let mut looked_at = 0;
        let files = markdown_files(&clone_path, &first, |count, _| looked_at = count).unwrap();
        assert_eq!(
            files,
            ["README.md", "docs/a.MD", "docs/b.md", "docs/odd name ü.md"]
        );
        assert_eq!(looked_at, 5);

        // A fetch moves the default branch, and the first commit still reads
        // the same.
        let second = testing::commit(&origin, &[("docs/c.md", "# C\n")]);
        fetch(&clone_path, &mut |_| ()).unwrap();
        assert_eq!(head(&clone_path).unwrap(), second);
        assert_eq!(
            markdown_files(&clone_path, &second, |_, _| ())
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            markdown_files(&clone_path, &first, |_, _| ())
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn skips_symbolic_links() {
        let root = TempDir::new("git-links");
        let origin = root.0.join("origin");
        testing::repo(&origin, &[("README.md", "# Readme\n")]);
        // A link entry added straight to the index, so the test needs no
        // link support from the file system.
        let blob = testing::git(&origin, &["hash-object", "-w", "README.md"]);
        testing::git(
            &origin,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("120000,{blob},LINK.md"),
            ],
        );
        testing::git(
            &origin,
            &["commit", "--quiet", "--message", "test: add link"],
        );
        let commit = testing::git(&origin, &["rev-parse", "HEAD"]);

        let clone_path = root.0.join("clone");
        clone(origin.to_str().unwrap(), &clone_path, &mut |_| ()).unwrap();
        let files = markdown_files(&clone_path, &commit, |_, _| ()).unwrap();
        assert_eq!(files, ["README.md"]);
    }

    #[test]
    fn explains_a_repository_without_commits() {
        let root = TempDir::new("git-empty");
        let origin = root.0.join("origin");
        std::fs::create_dir_all(&origin).unwrap();
        testing::git(&origin, &["init", "--quiet", "--initial-branch=main"]);
        let clone_path = root.0.join("clone");
        clone(origin.to_str().unwrap(), &clone_path, &mut |_| ()).unwrap();
        let err = head(&clone_path).unwrap_err();
        assert!(err.contains("no commits"), "{err}");
    }
}
