//! Progress on standard error while long work runs, so no stage looks
//! stuck. In a terminal, one line is redrawn in place a few times a second;
//! otherwise, such as when output goes to a file, a line is printed every few
//! seconds. Each stage ends with a line saying what it did and how long it
//! took. Modeled on web-of-trust's `progress.rs`.

use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

/// How often the line is redrawn in a terminal.
const TERMINAL_INTERVAL: Duration = Duration::from_millis(200);
/// How often a line is printed when standard error is not a terminal.
const LOG_INTERVAL: Duration = Duration::from_secs(10);

struct Stage {
    label: String,
    total: u64,
    /// What the stage counts, by its plural name, such as `files`.
    unit: &'static str,
    started: Instant,
    /// When the line was last shown.
    shown: Option<Instant>,
}

/// Reports stages of work and their progress.
pub struct Progress {
    /// Whether to write to standard error.
    enabled: bool,
    terminal: bool,
    stage: Option<Stage>,
    /// Whether a redrawn line is on screen and has to be cleared.
    drawn: bool,
}

impl Progress {
    /// Reports on standard error.
    pub fn to_stderr() -> Self {
        Progress {
            enabled: true,
            terminal: io::stderr().is_terminal(),
            stage: None,
            drawn: false,
        }
    }

    /// Reports nothing, for tests.
    #[cfg(test)]
    pub fn quiet() -> Self {
        Progress {
            enabled: false,
            terminal: false,
            stage: None,
            drawn: false,
        }
    }

    /// Starts a stage counting `unit` up to `total`, or with no total when 0.
    pub fn stage(&mut self, label: &str, total: u64, unit: &'static str) {
        self.clear();
        self.stage = Some(Stage {
            label: label.to_string(),
            total,
            unit,
            started: Instant::now(),
            shown: None,
        });
        if !self.enabled {
            return;
        }
        if self.terminal {
            // Shown until the first update replaces it, so stages with no
            // updates still say what is happening.
            let mut err = io::stderr().lock();
            let _ = write!(err, "\r\x1b[2K{label}...");
            let _ = err.flush();
            self.drawn = true;
        } else {
            eprintln!("{label}...");
        }
    }

    /// Reports how far the stage has come. `detail` names the current item
    /// and is only built when the line is shown, so calling this often costs
    /// little.
    pub fn update(&mut self, done: u64, detail: impl FnOnce() -> String) {
        if !self.enabled {
            return;
        }
        let Some(stage) = self.stage.as_mut() else {
            return;
        };
        let now = Instant::now();
        let interval = if self.terminal {
            TERMINAL_INTERVAL
        } else {
            LOG_INTERVAL
        };
        // The first line waits a moment, so quick stages stay quiet.
        let since = stage.shown.unwrap_or(stage.started);
        if now.duration_since(since) < interval {
            return;
        }
        stage.shown = Some(now);
        let line = progress_line(stage, done, &detail(), now);
        let mut err = io::stderr().lock();
        if self.terminal {
            // Back to the start of the line, and clear it.
            let _ = write!(err, "\r\x1b[2K{line}");
            self.drawn = true;
        } else {
            let _ = writeln!(err, "{line}");
        }
        let _ = err.flush();
    }

    /// Ends the stage with what it did, and how long it took.
    pub fn done(&mut self, summary: &str) {
        self.clear();
        let Some(stage) = self.stage.take() else {
            return;
        };
        if self.enabled {
            eprintln!(
                "{}: {summary} ({})",
                stage.label,
                elapsed(stage.started.elapsed())
            );
        }
    }

    /// Prints a line of its own, outside any stage's progress.
    pub fn note(&mut self, message: &str) {
        self.clear();
        if self.enabled {
            eprintln!("{message}");
        }
    }

    /// Removes a redrawn line from the terminal.
    fn clear(&mut self) {
        if self.drawn {
            let mut err = io::stderr().lock();
            let _ = write!(err, "\r\x1b[2K");
            let _ = err.flush();
            self.drawn = false;
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.clear();
    }
}

fn progress_line(stage: &Stage, done: u64, detail: &str, now: Instant) -> String {
    let mut line = format!("{}: ", stage.label);
    if stage.total > 0 {
        let percent = (done as f64 * 100.0 / stage.total as f64).min(100.0);
        line.push_str(&format!(
            "{percent:.0}% ({} of {} {})",
            thousands(done.min(stage.total)),
            thousands(stage.total),
            stage.unit
        ));
    } else {
        line.push_str(&format!("{} {}", thousands(done), stage.unit));
    }
    if !detail.is_empty() {
        line.push_str(", ");
        line.push_str(detail);
    }
    line.push_str(&format!(", {}", elapsed(now.duration_since(stage.started))));
    line
}

/// A count with thousands separators, such as `1,942`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A count with its noun, such as `1 file` or `1,942 files`.
pub fn counted(n: u64, singular: &str, plural: &str) -> String {
    format!(
        "{} {}",
        thousands(n),
        if n == 1 { singular } else { plural }
    )
}

/// A duration such as `0.4s`, `12s` or `3m 05s`.
pub fn elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds < 10.0 {
        format!("{seconds:.1}s")
    } else if seconds < 60.0 {
        format!("{seconds:.0}s")
    } else {
        let whole = seconds as u64;
        format!("{}m {:02}s", whole / 60, whole % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_counts_and_durations() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_942), "1,942");
        assert_eq!(thousands(1_000_000), "1,000,000");
        assert_eq!(counted(1, "file", "files"), "1 file");
        assert_eq!(counted(0, "file", "files"), "0 files");
        assert_eq!(counted(1_942, "file", "files"), "1,942 files");
        assert_eq!(elapsed(Duration::from_millis(400)), "0.4s");
        assert_eq!(elapsed(Duration::from_secs(12)), "12s");
        assert_eq!(elapsed(Duration::from_secs(185)), "3m 05s");
    }

    #[test]
    fn describes_progress_with_and_without_a_total() {
        let started = Instant::now();
        let stage = |total| Stage {
            label: "Listing files".into(),
            total,
            unit: "files",
            started,
            shown: None,
        };
        let line = progress_line(&stage(4_000), 1_000, "docs/a.md", started);
        assert_eq!(
            line,
            "Listing files: 25% (1,000 of 4,000 files), docs/a.md, 0.0s"
        );
        let line = progress_line(&stage(0), 2_048, "", started);
        assert_eq!(line, "Listing files: 2,048 files, 0.0s");
    }

    #[test]
    fn stays_quiet_when_asked() {
        let mut progress = Progress::quiet();
        progress.note("nothing printed");
        progress.stage("Listing files", 10, "files");
        progress.update(5, || panic!("a quiet progress never builds its line"));
        progress.done("nothing to say");
    }
}
