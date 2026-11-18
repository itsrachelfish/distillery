//! Estimates: "Distill" counts the tokens of the selected files as a job,
//! and the Estimate page shows the expected cost before anything is sent to
//! a model. "Start run" there is what sends it.

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::get;
use axum::{Form, Router};
use maud::{Markup, html};
use serde::Deserialize;

use super::jobs::{self, Finished};
use super::layout::{self, dollars, number};
use super::sources as source_pages;
use super::{App, Flash, PageError, PageResult, failed, files, internal, runs, with_db};
use crate::anthropic::{self, Anthropic};
use crate::config;
use crate::db::{self, EstimateSection, RecordedEstimate, SectionKey, Source};
use crate::estimate;
use crate::git;
use crate::progress::counted;
use crate::prompts;
use crate::sections::{self, Section};

pub(super) fn routes() -> Router<App> {
    Router::new().route("/estimate", get(estimate_page).post(start_estimate))
}

/// The address of the page for one estimate.
pub(super) fn href(id: i64) -> String {
    href_many(&[id])
}

/// The address of the page for the estimates of several files together.
pub(super) fn href_many(ids: &[i64]) -> String {
    format!("/estimate?ids={}", join_ids(ids))
}

/// Estimate IDs as addresses and forms carry them: `3,4,5`.
pub(super) fn join_ids(ids: &[i64]) -> String {
    let ids: Vec<String> = ids.iter().map(i64::to_string).collect();
    ids.join(",")
}

/// Estimate IDs from an address or form.
pub(super) fn parse_ids(text: &str) -> Result<Vec<i64>, PageError> {
    let ids: Result<Vec<i64>, _> = text.split(',').map(|id| id.trim().parse()).collect();
    match ids {
        Ok(ids) if !ids.is_empty() => Ok(ids),
        _ => Err(PageError::not_found(
            "There are no estimates at this address.",
        )),
    }
}

/// The estimates with these IDs, which have to be files of one source at one
/// commit.
pub(super) fn find_all(
    conn: &rusqlite::Connection,
    ids: &[i64],
) -> Result<Vec<RecordedEstimate>, PageError> {
    let mut found: Vec<RecordedEstimate> = Vec::with_capacity(ids.len());
    for &id in ids {
        let recorded = db::find_estimate(conn, id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found(format!("There is no estimate {id}.")))?;
        if let Some(first) = found.first()
            && (first.estimate.source_id != recorded.estimate.source_id
                || first.estimate.commit_hash != recorded.estimate.commit_hash)
        {
            return Err(PageError::not_found(
                "These estimates are for different sources or commits.",
            ));
        }
        found.push(recorded);
    }
    Ok(found)
}

/// Whether an estimate was counted for the model and prompt in use.
pub(super) fn current(recorded: &RecordedEstimate, model: &str, prompt_version: &str) -> bool {
    recorded.estimate.model == model && recorded.estimate.prompt_version == prompt_version
}

/// Starts counting the tokens of the selected files in the background,
/// unless a job is running, and goes to the Job page. Files with an estimate
/// for the model and prompt in use aren't counted again.
///
/// The form holds `source`, `commit`, and one `path` per selected file, or
/// `all` for every Markdown file at the commit.
async fn start_estimate(
    State(app): State<App>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, PageError> {
    let mut source = None;
    let mut commit = None;
    let mut paths = Vec::new();
    let mut all = false;
    for (name, value) in fields {
        match name.as_str() {
            "source" => source = value.parse::<i64>().ok(),
            "commit" => commit = Some(value),
            "path" => paths.push(value),
            "all" => all = true,
            _ => {}
        }
    }
    let (Some(source), Some(commit)) = (source, commit) else {
        return Ok(failed("/", "The form named no source or commit."));
    };
    let back = match paths.as_slice() {
        [path] => files::href(source, &commit, path),
        _ => source_pages::href(source, Some(&commit)),
    };
    if paths.is_empty() && !all {
        return Ok(failed(&back, "Select at least one file."));
    }
    // A missing key is refused before a job starts, so the page says why.
    if app.model.api_key.is_empty() {
        return Ok(failed(&back, anthropic::NO_KEY));
    }

    let clone_dir = app.clone_dir.clone();
    let model = app.model.name.clone();
    let found = with_db(&app, move |conn| {
        let snapshot = source_pages::snapshot(conn, &clone_dir, source, Some(&commit))?;
        let paths = if all { snapshot.files.clone() } else { paths };
        let prompt = prompts::Extraction::load().map_err(PageError::internal)?;
        // Each file with the estimate it already has, if that is current.
        let mut wanted = Vec::with_capacity(paths.len());
        for path in paths {
            // Only the Markdown files listed at the commit can be read.
            if !snapshot.files.contains(&path) {
                return Err(PageError::not_found(format!(
                    "{path} is not a Markdown file at commit {}.",
                    snapshot.commit
                )));
            }
            let latest = db::latest_estimate(conn, snapshot.source.id, &snapshot.commit, &path)
                .map_err(internal)?
                .filter(|recorded| current(recorded, &model, &prompt.version))
                .map(|recorded| recorded.id);
            wanted.push((path, latest));
        }
        Ok((snapshot, wanted))
    })
    .await?;
    let (snapshot, wanted) = found;

    // With every estimate at hand there is nothing to count.
    if let Some(ids) = wanted
        .iter()
        .map(|(_, id)| *id)
        .collect::<Option<Vec<i64>>>()
    {
        return Ok(Redirect::to(&href_many(&ids)));
    }

    let title = match wanted.as_slice() {
        [(path, _)] => format!("Estimate for {path} in {}", snapshot.source.clone_folder),
        _ => format!(
            "Estimate for {} in {}",
            counted(wanted.len() as u64, "file", "files"),
            snapshot.source.clone_folder
        ),
    };
    let started = jobs::start(title, move |progress| {
        let mut conn = db::open(&app.db).map_err(|err| format!("Opening the database: {err}"))?;
        let mut contents = Vec::new();
        for (path, id) in &wanted {
            if id.is_none() {
                let text = git::file_at(&snapshot.clone, &snapshot.commit, path)?;
                contents.push((path.as_str(), text));
            }
        }
        let to_count: Vec<estimate::File> = contents
            .iter()
            .map(|(path, text)| estimate::File {
                source_id: snapshot.source.id,
                commit: &snapshot.commit,
                path: *path,
                contents: text.as_str(),
            })
            .collect();
        // The HTTPS client blocks, so it lives on the job's thread.
        let api = Anthropic::new(&app.model)?;
        let done = estimate::run(&mut conn, &api, &app.model, &to_count, progress)?;

        // The estimates of every selected file, in the order selected.
        let mut ids = Vec::new();
        for (path, id) in &wanted {
            let counted = done
                .estimates
                .iter()
                .find(|estimated| &estimated.estimate.path == path);
            if let Some(id) = id.or(counted.map(|estimated| estimated.id)) {
                ids.push(id);
            }
        }
        let tokens: i64 = done
            .estimates
            .iter()
            .map(|estimated| estimated.estimate.input_tokens)
            .sum();
        let sections: usize = done
            .estimates
            .iter()
            .map(|estimated| estimated.sections)
            .sum();
        let mut said = format!(
            "Counted {} in {}, at no cost.",
            counted(tokens as u64, "input token", "input tokens"),
            counted(sections as u64, "section", "sections")
        );
        if !done.empty.is_empty() {
            said.push_str(&format!(
                " Left out, without text to distill: {}.",
                done.empty.join(", ")
            ));
        }
        Ok(Finished {
            said,
            link: Some(("See the estimate".to_owned(), href_many(&ids))),
        })
    });
    Ok(if started {
        Redirect::to("/job")
    } else {
        failed("/job", jobs::BUSY)
    })
}

#[derive(Deserialize)]
struct EstimateQuery {
    ids: String,
}

async fn estimate_page(
    State(app): State<App>,
    Query(query): Query<EstimateQuery>,
    Query(flash): Query<Flash>,
) -> PageResult {
    let clone_dir = app.clone_dir.clone();
    let model = app.model.clone();
    with_db(&app, move |conn| {
        let ids = parse_ids(&query.ids)?;
        let found = find_all(conn, &ids)?;
        let first = &found[0].estimate;
        let source = db::find_source(conn, first.source_id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found("The estimate's source is gone."))?;
        let clone = clone_dir.join(&source.clone_folder);
        let prompt = prompts::Extraction::load().map_err(PageError::internal)?;

        let mut files = Vec::with_capacity(found.len());
        for recorded in &found {
            let estimate = &recorded.estimate;
            let counted = db::estimate_sections(conn, recorded.id).map_err(internal)?;
            // Sections that runs would skip, since the cache holds them.
            let blob_hash = git::blob_at(&clone, &estimate.commit_hash, &estimate.path)
                .map_err(PageError::internal)?;
            let mut cached = 0;
            for section in &counted {
                let key = SectionKey {
                    blob_hash: &blob_hash,
                    start_line: section.start_line,
                    end_line: section.end_line,
                    prompt_version: &estimate.prompt_version,
                    model: &estimate.model,
                };
                if db::cached_extraction(conn, &key)
                    .map_err(internal)?
                    .is_some()
                {
                    cached += 1;
                }
            }
            files.push(FileView {
                recorded,
                counted,
                cached,
            });
        }
        // Heading paths aren't stored: for one file, they come from the
        // file at the estimate's commit.
        let sections = match found.as_slice() {
            [only] => {
                let contents =
                    git::file_at(&clone, &only.estimate.commit_hash, &only.estimate.path)
                        .map_err(PageError::internal)?;
                sections::split(&contents)
            }
            _ => Vec::new(),
        };
        let view = View {
            source: &source,
            files: &files,
            sections: &sections,
            model: &model,
            ids: &ids,
            current: found
                .iter()
                .all(|recorded| current(recorded, &model.name, &prompt.version)),
        };
        let title = match found.as_slice() {
            [only] => format!("Estimate for {}", only.estimate.path),
            _ => format!(
                "Estimate for {}",
                counted(found.len() as u64, "file", "files")
            ),
        };
        Ok(layout::page(&title, &flash, page(&view)))
    })
    .await
}

/// One file's estimate.
struct FileView<'a> {
    recorded: &'a RecordedEstimate,
    /// The sections the estimate counted.
    counted: Vec<EstimateSection>,
    /// How many of them are distilled already.
    cached: i64,
}

/// What the page shows.
struct View<'a> {
    source: &'a Source,
    /// The estimates, at least one, all at one commit.
    files: &'a [FileView<'a>],
    /// The file's sections, for their numbers and heading paths. Empty for
    /// several files.
    sections: &'a [Section],
    /// The settings in use, for the run cost limit.
    model: &'a config::Model,
    ids: &'a [i64],
    /// Whether every estimate was counted for the model and prompt in use.
    current: bool,
}

fn page(view: &View) -> Markup {
    let source = view.source;
    let first = &view.files[0].recorded.estimate;
    let single = match view.files {
        [only] => Some(only),
        _ => None,
    };
    let estimates = || view.files.iter().map(|file| &file.recorded.estimate);
    let sections: usize = view.files.iter().map(|file| file.counted.len()).sum();
    let skipped: i64 = estimates().map(|estimate| estimate.skipped_sections).sum();
    let cached: i64 = view.files.iter().map(|file| file.cached).sum();
    let input_tokens: i64 = estimates().map(|estimate| estimate.input_tokens).sum();
    let input_cost: i64 = estimates().map(|estimate| estimate.input_cost_micros).sum();
    // Output is known only when every file has an expectation.
    let output: Option<(i64, i64)> = estimates()
        .map(|estimate| estimate.output_tokens.zip(estimate.output_cost_micros))
        .collect::<Option<Vec<(i64, i64)>>>()
        .map(|parts| {
            parts
                .iter()
                .fold((0, 0), |sum, part| (sum.0 + part.0, sum.1 + part.1))
        });
    let total = input_cost + output.map_or(0, |(_, cost)| cost);
    let limit = view.model.max_run_cost_micros();
    html! {
        p.crumbs {
            a href="/" { "Sources" }
            " / "
            a href=(source_pages::href(source.id, Some(&first.commit_hash))) {
                (source.clone_folder)
            }
            @if single.is_some() {
                " / "
                a href=(files::href(source.id, &first.commit_hash, &first.path)) { (first.path) }
            }
        }
        h1 { "Estimate" }
        p.lead {
            "Expected cost of distilling "
            @if single.is_some() {
                code { (first.path) }
            } @else {
                (counted(view.files.len() as u64, "file", "files"))
            }
            " at commit " (layout::commit(&first.commit_hash))
            ". Counting tokens is free, and nothing was sent to a model."
        }

        table {
            tbody {
                tr {
                    th scope="row" { "Model" }
                    td { code { (first.model) } " " span.note { (first.provider) } }
                }
                tr {
                    th scope="row" { "Prompt version" }
                    td { (layout::commit(&first.prompt_version)) }
                }
                @if let Some(only) = single {
                    tr {
                        th scope="row" { "Counted" }
                        td { (layout::utc(&only.recorded.created_at)) }
                    }
                }
                tr {
                    th scope="row" { "Sections to send" }
                    td {
                        (number(sections as i64))
                        @if skipped > 0 {
                            span.note {
                                " · " (number(skipped)) " skipped, holding a heading alone"
                            }
                        }
                    }
                }
                tr {
                    th scope="row" { "Sections cached" }
                    td {
                        (number(cached))
                        span.note { " · distilled already, so runs don't send them again" }
                    }
                }
                tr {
                    th scope="row" { "Input" }
                    td { (number(input_tokens)) " tokens · " (dollars(input_cost)) }
                }
                tr {
                    th scope="row" { "Output" }
                    td {
                        @match output {
                            Some((tokens, cost)) => {
                                "About " (number(tokens)) " tokens · " (dollars(cost))
                                span.note {
                                    " · from the output-to-input ratio of "
                                    (number(first.ratio_calls)) " earlier extraction calls"
                                }
                            },
                            None => {
                                "Not known yet"
                                span.note {
                                    " · expected output comes from the output-to-input ratio "
                                    "of earlier extraction calls, and this model had none"
                                }
                            },
                        }
                    }
                }
                tr {
                    th scope="row" { "Total" }
                    td {
                        strong { (dollars(total)) }
                        @if output.is_none() {
                            span.note { " · input alone" }
                        }
                        @if total > limit {
                            span.failed { " · over the run cost limit of " (dollars(limit)) }
                        } @else {
                            span.note { " · run cost limit " (dollars(limit)) }
                        }
                    }
                }
            }
        }
        p.note {
            "Costs come from the full input price configured when the tokens were counted, "
            "for every section. Runs skip cached sections and read the repeated prompt from "
            "the prompt cache when it applies, and record the tokens and cost of every call "
            "for comparison with this estimate."
        }

        h2 { "Run" }
        @if view.current {
            form.inline method="post" action="/run" {
                input type="hidden" name="estimates" value=(join_ids(view.ids));
                button type="submit" { "Start run" }
                span.note {
                    "Sends the sections that aren't cached to " code { (first.model) }
                    ". This costs money, and stops once the run passes " (dollars(limit)) "."
                }
            }
        } @else {
            p.note { (runs::STALE) }
        }

        @if let Some(only) = single {
            h2 { "Sections" }
            (section_table(view, only))
        } @else {
            h2 { "Files" }
            table {
                thead {
                    tr {
                        th scope="col" { "File" }
                        th.number scope="col" { "Sections" }
                        th.number scope="col" { "Cached" }
                        th.number scope="col" { "Input tokens" }
                        th.number scope="col" { "Input cost" }
                    }
                }
                tbody {
                    @for file in view.files {
                        @let estimate = &file.recorded.estimate;
                        tr {
                            td { a href=(href(file.recorded.id)) { (estimate.path) } }
                            td.number { (number(file.counted.len() as i64)) }
                            td.number { (number(file.cached)) }
                            td.number { (number(estimate.input_tokens)) }
                            td.number { (dollars(estimate.input_cost_micros)) }
                        }
                    }
                }
            }
        }
    }
}

/// One file's counted sections.
fn section_table(view: &View, only: &FileView) -> Markup {
    let estimate = &only.recorded.estimate;
    let file = files::href(view.source.id, &estimate.commit_hash, &estimate.path);
    // A counted section's number in the file and its heading path.
    let found = |row: &EstimateSection| {
        view.sections.iter().enumerate().find(|(_, section)| {
            section.start_line as i64 == row.start_line && section.end_line as i64 == row.end_line
        })
    };
    html! {
        table {
            caption.note {
                "Each count covers the prompt, the reply schema and the section, as one call "
                "sends them."
            }
            thead {
                tr {
                    th scope="col" { "Section" }
                    th scope="col" { "Lines" }
                    th scope="col" { "Heading path" }
                    th.number scope="col" { "Input tokens" }
                }
            }
            tbody {
                @for row in &only.counted {
                    tr {
                        @if let Some((position, section)) = found(row) {
                            td {
                                a href=(format!("{file}#S{}", position + 1)) {
                                    "Section " (position + 1)
                                }
                            }
                            td { (row.start_line) "–" (row.end_line) }
                            td { (section.title()) }
                        } @else {
                            td.note { "Section" }
                            td { (row.start_line) "–" (row.end_line) }
                            td.note { "Cut differently by the current section rules" }
                        }
                        td.number { (number(row.input_tokens)) }
                    }
                }
            }
        }
    }
}
