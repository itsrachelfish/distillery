//! Jobs: long work started from a page, such as the snapshot of a source.
//!
//! One job runs at a time, on its own thread, as plain synchronous code.
//! The Job page reads the job's shared progress report and reloads itself
//! until the job ends, with no script.

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Form, Router};
use maud::{Markup, html};
use serde::Deserialize;

use super::{App, Flash, failed, layout};
use crate::db;
use crate::git;
use crate::progress::{self, Progress, Report, SharedReport, counted};
use crate::snapshot;

/// How often the Job page reloads while a job runs.
const RELOAD_SECONDS: u32 = 2;

/// What a job says when it ends: what it did, or why it failed.
type Outcome = Result<String, String>;

/// The job under way, or the last one that ran.
struct Job {
    title: String,
    report: SharedReport,
    started: Instant,
    /// `None` while it runs; then the outcome and how long the job took.
    outcome: Option<(Outcome, Duration)>,
}

static JOB: Mutex<Option<Job>> = Mutex::new(None);

fn job() -> MutexGuard<'static, Option<Job>> {
    JOB.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether a job is under way.
pub(super) fn running() -> bool {
    job().as_ref().is_some_and(|job| job.outcome.is_none())
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/job", get(job_page))
        .route("/snapshot", post(start_snapshot))
}

/// What the page shows of a job.
struct Status {
    title: String,
    report: Report,
    elapsed: Duration,
    outcome: Option<Outcome>,
}

fn status() -> Option<Status> {
    let job = job();
    let job = job.as_ref()?;
    let report = job
        .report
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    Some(Status {
        title: job.title.clone(),
        report,
        elapsed: job
            .outcome
            .as_ref()
            .map_or_else(|| job.started.elapsed(), |(_, took)| *took),
        outcome: job.outcome.as_ref().map(|(outcome, _)| outcome.clone()),
    })
}

async fn job_page(Query(flash): Query<Flash>) -> Markup {
    let status = status();
    let running = status
        .as_ref()
        .is_some_and(|status| status.outcome.is_none());
    let body = page(status.as_ref());
    if running {
        layout::reloading_page("Job", &flash, body, RELOAD_SECONDS)
    } else {
        layout::page("Job", &flash, body)
    }
}

#[derive(Deserialize)]
struct SnapshotForm {
    url: String,
}

/// Starts the snapshot of a new or known source in the background, unless a
/// job is running, and goes to the Job page.
async fn start_snapshot(State(app): State<App>, Form(form): Form<SnapshotForm>) -> Redirect {
    // Bad URLs are refused before a job starts, so the form's page says why.
    let repo = match git::parse_url(&form.url) {
        Ok(repo) => repo,
        Err(err) => return failed("/", err),
    };

    let mut slot = job();
    if slot.as_ref().is_some_and(|job| job.outcome.is_none()) {
        return failed(
            "/job",
            "Another job is running. Start this one when it ends.",
        );
    }
    let report: SharedReport = Arc::default();
    *slot = Some(Job {
        title: format!("Snapshot of {}", repo.url),
        report: Arc::clone(&report),
        started: Instant::now(),
        outcome: None,
    });
    drop(slot);

    thread::spawn(move || {
        let mut progress = Progress::to_stderr_and(report);
        // A panic would otherwise leave the job running for good.
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            run_snapshot(&app.db, &app.clone_dir, &repo.url, &mut progress)
        }))
        .unwrap_or_else(|_| Err("The job stopped unexpectedly; see the terminal.".into()));
        if let Some(job) = job().as_mut() {
            let took = job.started.elapsed();
            job.outcome = Some((outcome, took));
        }
    });
    Redirect::to("/job")
}

fn run_snapshot(database: &Path, clone_dir: &Path, url: &str, progress: &mut Progress) -> Outcome {
    let conn = db::open(database).map_err(|err| format!("Opening the database: {err}"))?;
    let found = snapshot::run(&conn, clone_dir, url, progress).map_err(|err| err.to_string())?;
    Ok(format!(
        "Recorded commit {}, which {}. It has {}.",
        found.commit,
        if found.new_commit {
            "is new"
        } else {
            "an earlier snapshot recorded already"
        },
        counted(found.files.len() as u64, "Markdown file", "Markdown files")
    ))
}

fn page(status: Option<&Status>) -> Markup {
    html! {
        h1 { "Job" }
        @match status {
            None => {
                p.empty { "No job has run since the server started." }
            }
            Some(status) => (status_section(status)),
        }
        p { a href="/" { "Back to sources" } }
    }
}

fn status_section(status: &Status) -> Markup {
    let report = &status.report;
    html! {
        section.job {
            h2 {
                (status.title)
                span.muted {
                    @match &status.outcome {
                        None => ": running, ",
                        Some(Ok(_)) => ": finished, ",
                        Some(Err(_)) => ": failed, ",
                    }
                    (progress::elapsed(status.elapsed))
                }
            }
            @if !report.notes.is_empty() {
                ul {
                    @for note in &report.notes {
                        li { (note) }
                    }
                }
            }
            @if !report.done.is_empty() {
                table {
                    thead {
                        tr {
                            th scope="col" { "Stage" }
                            th scope="col" { "Result" }
                            th.number scope="col" { "Time" }
                        }
                    }
                    tbody {
                        @for stage in &report.done {
                            tr {
                                td { (stage.label) }
                                td { (stage.summary) }
                                td.number { (stage.took) }
                            }
                        }
                    }
                }
            }
            @if let (Some(current), None) = (&report.current, &status.outcome) {
                div.current-stage {
                    p { strong { (current.label) } }
                    @if current.total > 0 {
                        progress max=(current.total) value=(current.done.min(current.total)) {
                            (current.done.min(current.total) * 100 / current.total) "%"
                        }
                    } @else {
                        // With no total, the bar only shows that work goes on.
                        progress {}
                    }
                    p.muted { (current.line) }
                }
            }
            @match &status.outcome {
                None => p.note {
                    "This page reloads every " (RELOAD_SECONDS) " seconds until the job ends. "
                    "Progress also shows in the terminal running the server."
                },
                Some(Ok(said)) => p.flash.notice role="status" { (said) },
                Some(Err(err)) => p.flash.error role="alert" { (err) },
            }
        }
    }
}
