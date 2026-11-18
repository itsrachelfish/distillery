//! The File page: a Markdown file at a recorded commit, as source text
//! with line numbers, cut into its sections. Claims cite source lines, so
//! pages show the source and never render it as HTML. "Distill" starts
//! with an estimate of what the file would cost.

use axum::Router;
use axum::extract::{Query, State};
use axum::routing::get;
use maud::{Markup, html};
use serde::Deserialize;

use super::layout::{self, dollars, number};
use super::sources::{self as source_pages, Snapshot};
use super::{App, Flash, PageError, PageResult, estimates, internal, with_db};
use crate::db::{self, RecordedEstimate};
use crate::git;
use crate::progress::counted;
use crate::sections;

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
        Ok(layout::page(
            &query.path,
            &flash,
            page(&snapshot, &query.path, &contents, estimate.as_ref()),
        ))
    })
    .await
}

fn page(
    snapshot: &Snapshot,
    path: &str,
    contents: &str,
    estimate: Option<&RecordedEstimate>,
) -> Markup {
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
                    }
                }
            }
        }
    }
}
