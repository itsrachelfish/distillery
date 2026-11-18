//! Estimates: "Distill" on the File page counts a file's tokens as a job,
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

/// The address of an estimate's page.
pub(super) fn href(id: i64) -> String {
    format!("/estimate?id={id}")
}

#[derive(Deserialize)]
struct EstimateForm {
    source: i64,
    commit: String,
    path: String,
}

/// Starts counting a file's tokens in the background, unless a job is
/// running, and goes to the Job page.
async fn start_estimate(
    State(app): State<App>,
    Form(form): Form<EstimateForm>,
) -> Result<Redirect, PageError> {
    let back = files::href(form.source, &form.commit, &form.path);
    // A missing key is refused before a job starts, so the file's page says
    // why.
    if app.model.api_key.is_empty() {
        return Ok(failed(&back, anthropic::NO_KEY));
    }

    let clone_dir = app.clone_dir.clone();
    let snapshot = with_db(&app, move |conn| {
        let snapshot = source_pages::snapshot(conn, &clone_dir, form.source, Some(&form.commit))?;
        // Only the Markdown files listed at the commit can be read.
        if !snapshot.files.contains(&form.path) {
            return Err(PageError::not_found(format!(
                "{} is not a Markdown file at commit {}.",
                form.path, snapshot.commit
            )));
        }
        Ok((snapshot, form.path))
    })
    .await?;
    let (snapshot, path) = snapshot;

    let title = format!("Estimate for {path} in {}", snapshot.source.clone_folder);
    let started = jobs::start(title, move |progress| {
        let mut conn = db::open(&app.db).map_err(|err| format!("Opening the database: {err}"))?;
        let contents = git::file_at(&snapshot.clone, &snapshot.commit, &path)?;
        // The HTTPS client blocks, so it lives on the job's thread.
        let api = Anthropic::new(&app.model)?;
        let file = estimate::File {
            source_id: snapshot.source.id,
            commit: &snapshot.commit,
            path: &path,
            contents: &contents,
        };
        let done = estimate::run(&mut conn, &api, &app.model, &file, progress)?;
        Ok(Finished {
            said: format!(
                "Counted {} in {}, at no cost: {} as input.",
                counted(
                    done.estimate.input_tokens as u64,
                    "input token",
                    "input tokens"
                ),
                counted(done.sections as u64, "section", "sections"),
                dollars(done.estimate.input_cost_micros)
            ),
            link: Some(("See the estimate".to_owned(), href(done.id))),
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
    id: i64,
}

async fn estimate_page(
    State(app): State<App>,
    Query(query): Query<EstimateQuery>,
    Query(flash): Query<Flash>,
) -> PageResult {
    let clone_dir = app.clone_dir.clone();
    let model = app.model.clone();
    with_db(&app, move |conn| {
        let recorded = db::find_estimate(conn, query.id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found("There is no estimate with this ID."))?;
        let counted = db::estimate_sections(conn, query.id).map_err(internal)?;
        let estimate = &recorded.estimate;
        let source = db::find_source(conn, estimate.source_id)
            .map_err(internal)?
            .ok_or_else(|| PageError::not_found("The estimate's source is gone."))?;
        // Heading paths aren't stored: they come from the file at the
        // estimate's commit.
        let clone = clone_dir.join(&source.clone_folder);
        let contents = git::file_at(&clone, &estimate.commit_hash, &estimate.path)
            .map_err(PageError::internal)?;
        let sections = sections::split(&contents);
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
        let prompt = prompts::Extraction::load().map_err(PageError::internal)?;
        let view = View {
            recorded: &recorded,
            source: &source,
            counted: &counted,
            sections: &sections,
            model: &model,
            cached,
            current: estimate.model == model.name && estimate.prompt_version == prompt.version,
        };
        Ok(layout::page(
            &format!("Estimate for {}", estimate.path),
            &flash,
            page(&view),
        ))
    })
    .await
}

/// What the page shows.
struct View<'a> {
    recorded: &'a RecordedEstimate,
    source: &'a Source,
    /// The sections the estimate counted.
    counted: &'a [EstimateSection],
    /// The file's sections, for their numbers and heading paths.
    sections: &'a [Section],
    /// The settings in use, for the run cost limit.
    model: &'a config::Model,
    /// How many of the counted sections are distilled already.
    cached: i64,
    /// Whether the estimate was counted for the model and prompt in use.
    current: bool,
}

fn page(view: &View) -> Markup {
    let estimate = &view.recorded.estimate;
    let source = view.source;
    let file = files::href(source.id, &estimate.commit_hash, &estimate.path);
    let total = estimate.input_cost_micros + estimate.output_cost_micros.unwrap_or(0);
    let limit = view.model.max_run_cost_micros();
    // A counted section's number in the file and its heading path.
    let found = |row: &EstimateSection| {
        view.sections.iter().enumerate().find(|(_, section)| {
            section.start_line as i64 == row.start_line && section.end_line as i64 == row.end_line
        })
    };
    html! {
        p.crumbs {
            a href="/" { "Sources" }
            " / "
            a href=(source_pages::href(source.id, Some(&estimate.commit_hash))) {
                (source.clone_folder)
            }
            " / "
            a href=(file) { (estimate.path) }
        }
        h1 { "Estimate" }
        p.lead {
            "Expected cost of distilling " code { (estimate.path) } " at commit "
            (layout::commit(&estimate.commit_hash))
            ". Counting tokens is free, and nothing was sent to a model."
        }

        table {
            tbody {
                tr {
                    th scope="row" { "Model" }
                    td { code { (estimate.model) } " " span.note { (estimate.provider) } }
                }
                tr {
                    th scope="row" { "Prompt version" }
                    td { (layout::commit(&estimate.prompt_version)) }
                }
                tr {
                    th scope="row" { "Counted" }
                    td { (layout::utc(&view.recorded.created_at)) }
                }
                tr {
                    th scope="row" { "Sections to send" }
                    td {
                        (number(view.counted.len() as i64))
                        @if estimate.skipped_sections > 0 {
                            span.note {
                                " · " (number(estimate.skipped_sections))
                                " skipped, holding a heading alone"
                            }
                        }
                    }
                }
                tr {
                    th scope="row" { "Sections cached" }
                    td {
                        (number(view.cached))
                        span.note { " · distilled already, so runs don't send them again" }
                    }
                }
                tr {
                    th scope="row" { "Input" }
                    td {
                        (number(estimate.input_tokens)) " tokens · "
                        (dollars(estimate.input_cost_micros))
                    }
                }
                tr {
                    th scope="row" { "Output" }
                    td {
                        @match (estimate.output_tokens, estimate.output_cost_micros) {
                            (Some(tokens), Some(cost)) => {
                                "About " (number(tokens)) " tokens · " (dollars(cost))
                                span.note {
                                    " · from the output-to-input ratio of "
                                    (number(estimate.ratio_calls)) " earlier extraction calls"
                                }
                            },
                            _ => {
                                "Not known yet"
                                span.note {
                                    " · expected output comes from the output-to-input ratio "
                                    "of earlier extraction calls, and this model has none"
                                }
                            },
                        }
                    }
                }
                tr {
                    th scope="row" { "Total" }
                    td {
                        strong { (dollars(total)) }
                        @if estimate.output_cost_micros.is_none() {
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
            "Costs come from the prices configured when the tokens were counted. Token counts "
            "are the provider's estimates: runs record the tokens and cost of every call, for "
            "comparison with this estimate."
        }

        h2 { "Sections" }
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
                @for row in view.counted {
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

        h2 { "Run" }
        @if view.current {
            form.inline method="post" action="/run" {
                input type="hidden" name="estimate" value=(view.recorded.id);
                button type="submit" { "Start run" }
                span.note {
                    "Sends the sections that aren't cached to " code { (estimate.model) }
                    ". This costs money, and stops once the run passes " (dollars(limit)) "."
                }
            }
        } @else {
            p.note { (runs::STALE) }
        }
    }
}
