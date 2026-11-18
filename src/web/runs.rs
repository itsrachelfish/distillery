//! Runs: "Start run" on the Estimate page sends a file's sections to the
//! model as a job, and the Run page shows what came back beside what the
//! estimate expected.

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::get;
use axum::{Form, Router};
use maud::{Markup, html};
use serde::Deserialize;

use super::jobs::{self, Finished};
use super::layout::{self, dollars, number};
use super::sources as source_pages;
use super::{App, Flash, PageError, PageResult, estimates, failed, files, internal, with_db};
use crate::anthropic::{self, Anthropic};
use crate::db::{
    self, Extraction, RecordedEstimate, RecordedRun, SectionKey, SectionUsage, Source,
};
use crate::estimate::heading_only;
use crate::extract;
use crate::git;
use crate::progress::counted;
use crate::prompts;
use crate::sections;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/run", get(run_page).post(start_run))
}

/// The address of a run's page.
pub(super) fn href(id: i64) -> String {
    format!("/run?id={id}")
}

/// What estimates and runs say when the model or prompt changed since an
/// estimate was counted.
pub(super) const STALE: &str =
    "The model or prompt changed since this estimate. Distill the file again for a current one.";

#[derive(Deserialize)]
struct StartForm {
    estimate: i64,
}

/// Starts a run from an estimate in the background, unless a job is
/// running, and goes to the Job page. This is where money is spent.
async fn start_run(
    State(app): State<App>,
    Form(form): Form<StartForm>,
) -> Result<Redirect, PageError> {
    let back = estimates::href(form.estimate);
    if app.model.api_key.is_empty() {
        return Ok(failed(&back, anthropic::NO_KEY));
    }

    let clone_dir = app.clone_dir.clone();
    let model = app.model.name.clone();
    let estimate_id = form.estimate;
    let found = with_db(&app, move |conn| {
        let recorded = db::find_estimate(conn, estimate_id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found("There is no estimate with this ID."))?;
        let estimate = recorded.estimate;
        let snapshot = source_pages::snapshot(
            conn,
            &clone_dir,
            estimate.source_id,
            Some(&estimate.commit_hash),
        )?;
        // Only the Markdown files listed at the commit can be read.
        if !snapshot.files.contains(&estimate.path) {
            return Err(PageError::not_found(format!(
                "{} is not a Markdown file at commit {}.",
                estimate.path, snapshot.commit
            )));
        }
        let current = prompts::Extraction::load().map_err(PageError::internal)?;
        let stale = estimate.model != model || estimate.prompt_version != current.version;
        Ok((snapshot, estimate.path, stale))
    })
    .await?;
    let (snapshot, path, stale) = found;
    // Runs cost what their estimate said only for the same model and prompt.
    if stale {
        return Ok(failed(&back, STALE));
    }

    let title = format!("Run for {path} in {}", snapshot.source.clone_folder);
    let started = jobs::start(title, move |progress| {
        let mut conn = db::open(&app.db).map_err(|err| format!("Opening the database: {err}"))?;
        let contents = git::file_at(&snapshot.clone, &snapshot.commit, &path)?;
        let blob_hash = git::blob_at(&snapshot.clone, &snapshot.commit, &path)?;
        // The HTTPS client blocks, so it lives on the job's thread.
        let api = Anthropic::new(&app.model)?;
        let file = extract::File {
            source_id: snapshot.source.id,
            commit: &snapshot.commit,
            path: &path,
            blob_hash: &blob_hash,
            contents: &contents,
        };
        let ran = extract::run(
            &mut conn,
            &api,
            &app.model,
            &file,
            Some(estimate_id),
            progress,
        )?;
        Ok(Finished {
            said: format!(
                "{} from {} for {}. Not distilled: {}. From the cache: {}. Not sent: {}.",
                counted(ran.claims as u64, "claim", "claims"),
                counted(ran.distilled as u64, "section", "sections"),
                dollars(ran.cost_micros),
                ran.failed,
                ran.cached,
                ran.unsent
            ),
            link: Some(("See the run".to_owned(), href(ran.run_id))),
        })
    });
    Ok(if started {
        Redirect::to("/job")
    } else {
        failed("/job", jobs::BUSY)
    })
}

#[derive(Deserialize)]
struct RunQuery {
    id: i64,
}

async fn run_page(
    State(app): State<App>,
    Query(query): Query<RunQuery>,
    Query(flash): Query<Flash>,
) -> PageResult {
    let clone_dir = app.clone_dir.clone();
    with_db(&app, move |conn| {
        let recorded = db::find_run(conn, query.id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found("There is no run with this ID."))?;
        let run = &recorded.run;
        let source = db::find_source(conn, run.source_id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found("The run's source is gone."))?;
        let (path, blob_hash) = db::run_file(conn, query.id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found("The run has no file."))?;
        let usage = db::run_usage(conn, query.id).map_err(internal)?;
        let extractions = db::run_extractions(conn, query.id).map_err(internal)?;
        let estimate = match run.estimate_id {
            Some(id) => db::find_estimate(conn, id).map_err(internal)?,
            None => None,
        };
        let estimated = match &estimate {
            Some(estimate) => db::estimate_sections(conn, estimate.id).map_err(internal)?,
            None => Vec::new(),
        };

        // Heading paths aren't stored: they come from the file at the run's
        // commit.
        let clone = clone_dir.join(&source.clone_folder);
        let contents =
            git::file_at(&clone, &run.commit_hash, &path).map_err(PageError::internal)?;
        let mut rows = Vec::new();
        for (position, section) in sections::split(&contents).iter().enumerate() {
            if heading_only(section) {
                continue;
            }
            let (start, end) = (section.start_line as i64, section.end_line as i64);
            // The last result counts, for sections a run recorded twice.
            let result = extractions
                .iter()
                .rev()
                .find(|result| result.start_line == start && result.end_line == end)
                .cloned();
            let elsewhere = match result {
                Some(_) => false,
                None => {
                    let key = SectionKey {
                        blob_hash: &blob_hash,
                        start_line: start,
                        end_line: end,
                        prompt_version: &run.prompt_version,
                        model: &run.model,
                    };
                    db::cached_extraction(conn, &key)
                        .map_err(internal)?
                        .is_some()
                }
            };
            rows.push(Row {
                number: position + 1,
                start,
                end,
                title: section.title(),
                result,
                elsewhere,
                used: usage
                    .iter()
                    .find(|used| used.start_line == start && used.end_line == end)
                    .copied()
                    .unwrap_or_default(),
                estimated: estimated
                    .iter()
                    .find(|counted| counted.start_line == start && counted.end_line == end)
                    .map(|counted| counted.input_tokens),
            });
        }
        let view = View {
            recorded: &recorded,
            source: &source,
            path: &path,
            estimate: estimate.as_ref(),
            rows: &rows,
        };
        Ok(layout::page(
            &format!("Run {} of {path}", recorded.id),
            &flash,
            page(&view),
        ))
    })
    .await
}

/// One section of the run's file.
struct Row {
    /// The section's number in the file.
    number: usize,
    start: i64,
    end: i64,
    title: String,
    /// What the run recorded for the section, if it sent it.
    result: Option<Extraction>,
    /// Whether another run distilled the section, so this one had no need to.
    elsewhere: bool,
    /// What the run's calls for the section used.
    used: SectionUsage,
    /// The input tokens the estimate counted.
    estimated: Option<i64>,
}

/// What the page shows.
struct View<'a> {
    recorded: &'a RecordedRun,
    source: &'a Source,
    path: &'a str,
    estimate: Option<&'a RecordedEstimate>,
    rows: &'a [Row],
}

/// A share as a whole percentage, such as 25 for 1 of 4.
fn percent(part: i64, whole: i64) -> i64 {
    if whole == 0 {
        0
    } else {
        (part as f64 * 100.0 / whole as f64).round() as i64
    }
}

fn page(view: &View) -> Markup {
    let run = &view.recorded.run;
    let source = view.source;
    let file = files::href(source.id, &run.commit_hash, view.path);
    let estimate = view.estimate.map(|recorded| &recorded.estimate);
    let sum =
        |field: fn(&SectionUsage) -> i64| view.rows.iter().map(|row| field(&row.used)).sum::<i64>();
    let calls = sum(|used| used.calls);
    let failed_calls = sum(|used| used.failed);
    let input = sum(|used| used.input_tokens);
    let output = sum(|used| used.output_tokens);
    let thinking = sum(|used| used.thinking_tokens);
    let cost = sum(|used| used.cost_micros);
    html! {
        p.crumbs {
            a href="/" { "Sources" }
            " / "
            a href=(source_pages::href(source.id, Some(&run.commit_hash))) {
                (source.clone_folder)
            }
            " / "
            a href=(file) { (view.path) }
        }
        h1 { "Run " (view.recorded.id) }
        p.lead {
            "Claims extracted from " code { (view.path) } " at commit "
            (layout::commit(&run.commit_hash)) ". "
            a href=(file) { "See the claims under their sections" } "."
        }

        table {
            tbody {
                tr {
                    th scope="row" { "Outcome" }
                    td {
                        @match &view.recorded.outcome {
                            Some(outcome) => (outcome),
                            None => span.note { "Not finished" },
                        }
                    }
                }
                tr {
                    th scope="row" { "Model" }
                    td { code { (run.model) } " " span.note { (run.provider) } }
                }
                tr {
                    th scope="row" { "Prompt version" }
                    td { (layout::commit(&run.prompt_version)) }
                }
                tr {
                    th scope="row" { "Started" }
                    td { (layout::utc(&view.recorded.started_at)) }
                }
            }
        }

        h2 { "Estimate and actual" }
        table {
            thead {
                tr {
                    th scope="col" { "" }
                    th.number scope="col" {
                        @match view.estimate {
                            Some(recorded) => a href=(estimates::href(recorded.id)) { "Estimate" },
                            None => "Estimate",
                        }
                    }
                    th.number scope="col" { "Actual" }
                }
            }
            tbody {
                tr {
                    th scope="row" { "Calls" }
                    td.number {
                        @match view.estimate {
                            Some(_) => (number(
                                view.rows.iter().filter(|row| row.estimated.is_some()).count() as i64
                            )),
                            None => "–",
                        }
                    }
                    td.number {
                        (number(calls))
                        @if failed_calls > 0 {
                            span.failed { " · " (number(failed_calls)) " failed" }
                        }
                    }
                }
                tr {
                    th scope="row" { "Input tokens" }
                    td.number {
                        @match estimate {
                            Some(estimate) => (number(estimate.input_tokens)),
                            None => "–",
                        }
                    }
                    td.number { (number(input)) }
                }
                tr {
                    th scope="row" { "Output tokens" }
                    td.number {
                        @match estimate.and_then(|estimate| estimate.output_tokens) {
                            Some(tokens) => (number(tokens)),
                            None => span.note { "Not known" },
                        }
                    }
                    td.number { (number(output)) }
                }
                tr {
                    th scope="row" { "Thinking tokens, part of output" }
                    td.number { "–" }
                    td.number {
                        (number(thinking))
                        span.note { " · " (percent(thinking, output)) "% of output" }
                    }
                }
                tr {
                    th scope="row" { "Cost" }
                    td.number {
                        @match estimate {
                            Some(estimate) => {
                                (dollars(
                                    estimate.input_cost_micros
                                        + estimate.output_cost_micros.unwrap_or(0)
                                ))
                                @if estimate.output_cost_micros.is_none() {
                                    span.note { " · input alone" }
                                }
                            },
                            None => "–",
                        }
                    }
                    td.number { strong { (dollars(cost)) } }
                }
            }
        }
        p.note {
            "The estimate covers every section of the file, one call each. Runs skip sections "
            "that are already distilled, and send sections again after rejected claims or "
            "errors, so calls and tokens can differ in both directions."
        }

        h2 { "Sections" }
        table {
            thead {
                tr {
                    th scope="col" { "Section" }
                    th scope="col" { "Result" }
                    th.number scope="col" { "Calls" }
                    th.number scope="col" { "Input, estimate" }
                    th.number scope="col" { "Input" }
                    th.number scope="col" { "Output" }
                    th.number scope="col" { "Thinking" }
                    th.number scope="col" { "Cost" }
                }
            }
            tbody {
                @for row in view.rows {
                    tr {
                        td {
                            a href=(format!("{file}#S{}", row.number)) { "Section " (row.number) }
                            br;
                            span.note { "lines " (row.start) "–" (row.end) " · " (row.title) }
                        }
                        td {
                            @if let Some(result) = &row.result {
                                @if result.distilled {
                                    (counted(result.claims as u64, "claim", "claims"))
                                } @else {
                                    span.failed { "Not distilled" }
                                    @if let Some(reason) = &result.reason {
                                        @for line in reason.lines() {
                                            br;
                                            span.note { (line) }
                                        }
                                    }
                                    details {
                                        summary.note { "The model's reply" }
                                        pre.reply { (result.reply) }
                                    }
                                }
                            } @else if row.elsewhere {
                                span.note { "Distilled by another run" }
                            } @else {
                                span.note { "Not sent" }
                            }
                        }
                        td.number { (number(row.used.calls)) }
                        td.number {
                            @match row.estimated {
                                Some(tokens) => (number(tokens)),
                                None => "–",
                            }
                        }
                        td.number { (number(row.used.input_tokens)) }
                        td.number { (number(row.used.output_tokens)) }
                        td.number { (number(row.used.thinking_tokens)) }
                        td.number { (dollars(row.used.cost_micros)) }
                    }
                }
            }
        }
    }
}
