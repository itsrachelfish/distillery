//! Runs: "Start run" on the Estimate page sends the files' sections to the
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
    /// Estimate IDs, as `3,4,5`.
    estimates: String,
}

/// Starts a run from the estimates of one or more files in the background,
/// unless a job is running, and goes to the Job page. This is where money is
/// spent.
async fn start_run(
    State(app): State<App>,
    Form(form): Form<StartForm>,
) -> Result<Redirect, PageError> {
    let ids = estimates::parse_ids(&form.estimates)?;
    let back = estimates::href_many(&ids);
    if app.model.api_key.is_empty() {
        return Ok(failed(&back, anthropic::NO_KEY));
    }

    let clone_dir = app.clone_dir.clone();
    let model = app.model.name.clone();
    let found = with_db(&app, move |conn| {
        let found = estimates::find_all(conn, &ids)?;
        let first = &found[0].estimate;
        let snapshot =
            source_pages::snapshot(conn, &clone_dir, first.source_id, Some(&first.commit_hash))?;
        let prompt = prompts::Extraction::load().map_err(PageError::internal)?;
        let mut stale = false;
        let mut wanted = Vec::with_capacity(found.len());
        for recorded in &found {
            // Only the Markdown files listed at the commit can be read.
            if !snapshot.files.contains(&recorded.estimate.path) {
                return Err(PageError::not_found(format!(
                    "{} is not a Markdown file at commit {}.",
                    recorded.estimate.path, snapshot.commit
                )));
            }
            stale |= !estimates::current(recorded, &model, &prompt.version);
            wanted.push((recorded.estimate.path.clone(), recorded.id));
        }
        Ok((snapshot, wanted, stale))
    })
    .await?;
    let (snapshot, wanted, stale) = found;
    // Runs cost what their estimate said only for the same model and prompt.
    if stale {
        return Ok(failed(&back, STALE));
    }

    let title = match wanted.as_slice() {
        [(path, _)] => format!("Run for {path} in {}", snapshot.source.clone_folder),
        _ => format!(
            "Run for {} in {}",
            counted(wanted.len() as u64, "file", "files"),
            snapshot.source.clone_folder
        ),
    };
    let started = jobs::start(title, move |progress| {
        let mut conn = db::open(&app.db).map_err(|err| format!("Opening the database: {err}"))?;
        let mut read = Vec::with_capacity(wanted.len());
        for (path, _) in &wanted {
            let contents = git::file_at(&snapshot.clone, &snapshot.commit, path)?;
            let blob_hash = git::blob_at(&snapshot.clone, &snapshot.commit, path)?;
            read.push((contents, blob_hash));
        }
        let files: Vec<extract::File> = wanted
            .iter()
            .zip(&read)
            .map(|((path, estimate), (contents, blob_hash))| extract::File {
                path: path.as_str(),
                blob_hash: blob_hash.as_str(),
                contents: contents.as_str(),
                estimate_id: Some(*estimate),
            })
            .collect();
        // The HTTPS client blocks, so it lives on the job's thread.
        let api = Anthropic::new(&app.model)?;
        let ran = extract::run(
            &mut conn,
            &api,
            &app.model,
            snapshot.source.id,
            &snapshot.commit,
            &files,
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
        let run_files = db::run_files(conn, query.id).map_err(internal)?;
        let usage = db::run_usage(conn, query.id).map_err(internal)?;
        let extractions = db::run_extractions(conn, query.id).map_err(internal)?;
        let clone = clone_dir.join(&source.clone_folder);

        let mut estimates = Vec::new();
        let mut rows = Vec::new();
        for file in &run_files {
            let estimate = match file.estimate_id {
                Some(id) => db::find_estimate(conn, id).map_err(internal)?,
                None => None,
            };
            let estimated = match &estimate {
                Some(estimate) => db::estimate_sections(conn, estimate.id).map_err(internal)?,
                None => Vec::new(),
            };
            estimates.extend(estimate);
            // Heading paths aren't stored: they come from the file at the
            // run's commit.
            let contents =
                git::file_at(&clone, &run.commit_hash, &file.path).map_err(PageError::internal)?;
            for (position, section) in sections::split(&contents).iter().enumerate() {
                if heading_only(section) {
                    continue;
                }
                let (start, end) = (section.start_line as i64, section.end_line as i64);
                // The last result counts, for sections a run recorded twice.
                let result = extractions
                    .iter()
                    .rev()
                    .find(|result| {
                        result.blob_hash == file.blob_hash
                            && result.start_line == start
                            && result.end_line == end
                    })
                    .cloned();
                let elsewhere = match result {
                    Some(_) => false,
                    None => {
                        let key = SectionKey {
                            blob_hash: &file.blob_hash,
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
                    path: file.path.clone(),
                    number: position + 1,
                    start,
                    end,
                    title: section.title(),
                    result,
                    elsewhere,
                    used: usage
                        .iter()
                        .find(|used| {
                            used.path == file.path
                                && used.start_line == start
                                && used.end_line == end
                        })
                        .cloned()
                        .unwrap_or_default(),
                    estimated: estimated
                        .iter()
                        .find(|counted| counted.start_line == start && counted.end_line == end)
                        .map(|counted| counted.input_tokens),
                });
            }
        }
        let paths: Vec<&str> = run_files.iter().map(|file| file.path.as_str()).collect();
        let view = View {
            recorded: &recorded,
            source: &source,
            paths: &paths,
            files: run_files.len(),
            estimates: &estimates,
            rows: &rows,
        };
        Ok(layout::page(
            &format!("Run {}", recorded.id),
            &flash,
            page(&view),
        ))
    })
    .await
}

/// One section of one of the run's files.
struct Row {
    path: String,
    /// The section's number in its file.
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
    /// The run's files.
    paths: &'a [&'a str],
    /// How many files the run has.
    files: usize,
    /// The estimates the run's files started from.
    estimates: &'a [RecordedEstimate],
    /// Every section of every file, in file order.
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
    let file = |path: &str| files::href(source.id, &run.commit_hash, path);
    let sum =
        |field: fn(&SectionUsage) -> i64| view.rows.iter().map(|row| field(&row.used)).sum::<i64>();
    let calls = sum(|used| used.calls);
    let failed_calls = sum(|used| used.failed);
    let input = sum(|used| used.input_tokens);
    let cached = sum(|used| used.cached_tokens);
    let output = sum(|used| used.output_tokens);
    let thinking = sum(|used| used.thinking_tokens);
    let cost = sum(|used| used.cost_micros);
    let claims: i64 = view
        .rows
        .iter()
        .filter_map(|row| row.result.as_ref())
        .map(|result| result.claims)
        .sum();

    // What the estimates expected, when every file started from one.
    let estimates = || view.estimates.iter().map(|recorded| &recorded.estimate);
    let estimated = view.estimates.len() == view.files && view.files > 0;
    let estimated_calls = view
        .rows
        .iter()
        .filter(|row| row.estimated.is_some())
        .count();
    let estimated_input: i64 = estimates().map(|estimate| estimate.input_tokens).sum();
    let estimated_cost: i64 = estimates()
        .map(|estimate| estimate.input_cost_micros + estimate.output_cost_micros.unwrap_or(0))
        .sum();
    let estimated_output: Option<i64> = estimates().map(|estimate| estimate.output_tokens).sum();
    let ids: Vec<i64> = view.estimates.iter().map(|recorded| recorded.id).collect();
    // The path of a run's file, for runs of one file.
    let only_path = match view.files {
        1 => view.rows.first().map(|row| row.path.as_str()),
        _ => None,
    };
    html! {
        p.crumbs {
            a href="/" { "Sources" }
            " / "
            a href=(source_pages::href(source.id, Some(&run.commit_hash))) {
                (source.clone_folder)
            }
            @if let Some(path) = only_path {
                " / "
                a href=(file(path)) { (path) }
            }
        }
        h1 { "Run " (view.recorded.id) }
        p.lead {
            (counted(claims as u64, "claim", "claims")) " extracted from "
            (counted(view.files as u64, "file", "files")) " at commit "
            (layout::commit(&run.commit_hash))
            ". Claims show under their sections on each file's page."
        }
        form.inline method="post" action="/group" {
            input type="hidden" name="source" value=(source.id);
            input type="hidden" name="commit" value=(run.commit_hash);
            @for path in view.paths {
                input type="hidden" name="path" value=(path);
            }
            button type="submit" { "Group claims" }
            span.note {
                "Groups each file's claims into compound claims: one call to the model per "
                "file, none for files grouped before with the same claims or with fewer than "
                (crate::group::FEWEST_CLAIMS) " claims."
            }
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
                        @if estimated {
                            a href=(estimates::href_many(&ids)) { "Estimate" }
                        } @else {
                            "Estimate"
                        }
                    }
                    th.number scope="col" { "Actual" }
                }
            }
            tbody {
                tr {
                    th scope="row" { "Calls" }
                    td.number {
                        @if estimated { (number(estimated_calls as i64)) } @else { "–" }
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
                        @if estimated { (number(estimated_input)) } @else { "–" }
                    }
                    td.number {
                        (number(input))
                        @if cached > 0 {
                            span.note { " · " (percent(cached, input)) "% from the prompt cache" }
                        }
                    }
                }
                tr {
                    th scope="row" { "Output tokens" }
                    td.number {
                        @match estimated_output.filter(|_| estimated) {
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
                        @if estimated {
                            (dollars(estimated_cost))
                            @if estimated_output.is_none() {
                                span.note { " · input alone" }
                            }
                        } @else {
                            "–"
                        }
                    }
                    td.number { strong { (dollars(cost)) } }
                }
            }
        }
        p.note {
            "The estimate covers every section of the files, one call each at the full input "
            "price. Runs skip sections that are already distilled, read the repeated prompt "
            "from the prompt cache when it applies, and send sections again after rejected "
            "claims or errors, so calls, tokens and cost can differ in both directions."
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
                @for group in view.rows.chunk_by(|a, b| a.path == b.path) {
                    tr {
                        th colspan="8" scope="colgroup" {
                            a href=(file(&group[0].path)) { (group[0].path) }
                        }
                    }
                    @for row in group {
                        tr {
                            td {
                                a href=(format!("{}#S{}", file(&row.path), row.number)) {
                                    "Section " (row.number)
                                }
                                br;
                                span.note { "lines " (row.start) "–" (row.end) " · " (row.title) }
                            }
                            td { (result(row)) }
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
}

/// What a run did with a section.
fn result(row: &Row) -> Markup {
    html! {
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
}
