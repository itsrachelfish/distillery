//! The File page: a Markdown file at a recorded commit, as source text
//! with line numbers, cut into its sections. Claims cite source lines, so
//! pages show the source and never render it as HTML. "Distill" starts
//! with an estimate of what the file would cost, and extracted claims show
//! under their sections.

use axum::Router;
use axum::extract::{Query, State};
use axum::routing::get;
use maud::{Markup, html};
use serde::Deserialize;

use super::layout::{self, dollars, number};
use super::sources::{self as source_pages, Snapshot};
use super::{App, Flash, PageError, PageResult, estimates, internal, runs, with_db};
use crate::db::{self, RecordedEstimate, ShownClaim};
use crate::git;
use crate::progress::counted;
use crate::sections::{self, Section};

pub(super) fn routes() -> Router<App> {
    Router::new().route("/file", get(file_page))
}

#[derive(Deserialize)]
struct FileQuery {
    source: i64,
    commit: String,
    path: String,
}

/// The address of a file's page.
pub(super) fn href(source_id: i64, commit: &str, path: &str) -> String {
    format!(
        "/file?source={source_id}&commit={commit}&path={}",
        layout::url_encode(path)
    )
}

async fn file_page(
    State(app): State<App>,
    Query(query): Query<FileQuery>,
    Query(flash): Query<Flash>,
) -> PageResult {
    let clone_dir = app.clone_dir.clone();
    with_db(&app, move |conn| {
        let snapshot = source_pages::snapshot(conn, &clone_dir, query.source, Some(&query.commit))?;
        // Only the Markdown files listed at the commit can be read, so the
        // address can't name anything else in the repository.
        if !snapshot.files.contains(&query.path) {
            return Err(PageError::not_found(format!(
                "{} is not a Markdown file at commit {}.",
                query.path, snapshot.commit
            )));
        }
        let contents = git::file_at(&snapshot.clone, &snapshot.commit, &query.path)
            .map_err(PageError::internal)?;
        let estimate = db::latest_estimate(conn, snapshot.source.id, &snapshot.commit, &query.path)
            .map_err(internal)?;
        let run = db::latest_run(conn, snapshot.source.id, &snapshot.commit, &query.path)
            .map_err(internal)?;
        // Claims belong to the file's contents, whichever commit they were
        // extracted at.
        let blob_hash = git::blob_at(&snapshot.clone, &snapshot.commit, &query.path)
            .map_err(PageError::internal)?;
        let claims = db::claims_of_blob(conn, &blob_hash).map_err(internal)?;
        let view = View {
            snapshot: &snapshot,
            path: &query.path,
            contents: &contents,
            estimate: estimate.as_ref(),
            run,
            claims: &claims,
        };
        Ok(layout::page(&query.path, &flash, page(&view)))
    })
    .await
}

/// What the page shows.
struct View<'a> {
    snapshot: &'a Snapshot,
    path: &'a str,
    contents: &'a str,
    /// The file's latest estimate.
    estimate: Option<&'a RecordedEstimate>,
    /// The ID of the file's latest run.
    run: Option<i64>,
    /// The claims extracted from the file's contents.
    claims: &'a [ShownClaim],
}

fn page(view: &View) -> Markup {
    let View {
        snapshot,
        path,
        contents,
        estimate,
        ..
    } = *view;
    let source = &snapshot.source;
    let lines: Vec<&str> = contents.lines().collect();
    let sections = sections::split(contents);
    // The section starting at each line, by its number among the sections.
    let starts = |line: usize| {
        sections
            .iter()
            .enumerate()
            .find(|(_, section)| section.start_line == line)
    };
    // The section ending at each line.
    let ends = |line: usize| sections.iter().find(|section| section.end_line == line);
    let claims_of = |section: &Section| -> Vec<&ShownClaim> {
        view.claims
            .iter()
            .filter(|shown| shown.section_start == section.start_line as i64)
            .collect()
    };
    html! {
        p.crumbs {
            a href="/" { "Sources" }
            " / "
            a href=(source_pages::href(source.id, Some(&snapshot.commit))) {
                (source.clone_folder)
            }
        }
        h1 { (path) }
        p.note {
            "At commit " (layout::commit(&snapshot.commit)) ", "
            (counted(lines.len() as u64, "line", "lines")) " in "
            (counted(sections.len() as u64, "section", "sections")) "."
        }
        p.note {
            "Sections run from one heading to the next. Each is sent to the model on its own, "
            "with the headings above it as context."
        }
        @if lines.is_empty() {
            p.empty { "This file is empty." }
        } @else {
            form.inline method="post" action="/estimate" {
                input type="hidden" name="source" value=(source.id);
                input type="hidden" name="commit" value=(snapshot.commit);
                input type="hidden" name="path" value=(path);
                button type="submit" { "Distill" }
                span.note {
                    "Starts with an estimate: tokens are counted section by section, which "
                    "is free, and the expected cost is shown before anything runs."
                }
            }
            @if let Some(recorded) = estimate {
                p.note {
                    "Latest estimate: "
                    a href=(estimates::href(recorded.id)) {
                        (number(recorded.estimate.input_tokens)) " input tokens, "
                        (dollars(recorded.estimate.input_cost_micros))
                    }
                    ", counted " (layout::utc(&recorded.created_at)) "."
                    @if let Some(run) = view.run {
                        " Latest run: " a href=(runs::href(run)) { "run " (run) } "."
                    }
                }
            }
            @if !view.claims.is_empty() {
                p.note {
                    (counted(view.claims.len() as u64, "claim", "claims"))
                    " extracted, shown under their sections. Every claim is an AI proposal "
                    "until a reviewer confirms it."
                }
            }
            table.lines {
                tbody {
                    @for (index, line) in lines.iter().enumerate() {
                        @let number = index + 1;
                        @if let Some((position, section)) = starts(number) {
                            tr.section-start id=(format!("S{}", position + 1)) {
                                td colspan="2" {
                                    strong { "Section " (position + 1) }
                                    " · lines " (section.start_line) "–" (section.end_line)
                                    " · " (section.title())
                                }
                            }
                        }
                        tr id=(format!("L{number}")) {
                            td.line-number { a href=(format!("#L{number}")) { (number) } }
                            td.line-text { (line) }
                        }
                        @if let Some(section) = ends(number) {
                            @let found = claims_of(section);
                            @if !found.is_empty() {
                                tr.claims {
                                    td colspan="2" { (claims(&found)) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A section's claims, each with its kind and the lines of its quote.
fn claims(found: &[&ShownClaim]) -> Markup {
    html! {
        p.note {
            strong { "AI proposal" }
            " · " (counted(found.len() as u64, "claim", "claims"))
            @if let Some(first) = found.first() {
                " from " code { (first.model) } ", prompt version "
                (layout::commit(&first.prompt_version))
            }
        }
        ul {
            @for shown in found {
                @let claim = &shown.claim;
                li id=(format!("C{}", shown.id)) {
                    span.kind { (claim.kind) }
                    @if claim.illustrative {
                        " " span.kind { "illustrative" }
                    }
                    " " (claim.text)
                    br;
                    span.note {
                        a href=(format!("#L{}", claim.start_line)) {
                            @if claim.start_line == claim.end_line {
                                "line " (claim.start_line)
                            } @else {
                                "lines " (claim.start_line) "–" (claim.end_line)
                            }
                        }
                        " · " q { (claim.quote) }
                    }
                }
            }
        }
    }
}
