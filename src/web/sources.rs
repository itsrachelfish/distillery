//! The Sources page, with every repository added and its latest snapshot,
//! and the Source page, with one repository's recorded commits and the
//! Markdown files at one of them.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::{Query, State};
use axum::routing::get;
use maud::{Markup, html};
use rusqlite::Connection;
use serde::Deserialize;

use super::{App, Flash, PageError, PageResult, files, internal, layout, with_db};
use crate::db::{self, Revision, Source, SourceSummary};
use crate::git;
use crate::progress::counted;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/", get(sources_page))
        .route("/source", get(source_page))
}

/// The address of a source's page, showing the files at `commit`, or at the
/// latest commit when `None`.
pub(super) fn href(source_id: i64, commit: Option<&str>) -> String {
    match commit {
        Some(commit) => format!("/source?id={source_id}&commit={commit}"),
        None => format!("/source?id={source_id}"),
    }
}

/// A source as its pages show it: at one recorded commit, with the Markdown
/// files there.
pub(super) struct Snapshot {
    pub source: Source,
    /// Where the bare clone lives.
    pub clone: PathBuf,
    /// Every recorded commit, latest first.
    pub revisions: Vec<Revision>,
    /// The full hash of the commit shown.
    pub commit: String,
    /// Markdown files at the commit, sorted by path.
    pub files: Vec<String>,
}

/// Reads a source at one of its recorded commits, or at the latest one when
/// `commit` is `None`. Only recorded commits are read, so addresses can't
/// name anything else in the clone.
pub(super) fn snapshot(
    conn: &Connection,
    clone_dir: &Path,
    source_id: i64,
    commit: Option<&str>,
) -> Result<Snapshot, PageError> {
    let source = db::find_source(conn, source_id)
        .map_err(internal)?
        .ok_or_else(|| PageError::not_found("There is no source with this ID."))?;
    let revisions = db::revisions(conn, source_id).map_err(internal)?;
    let shown = match commit {
        Some(commit) => revisions
            .iter()
            .find(|revision| revision.commit_hash == commit),
        None => revisions.first(),
    };
    let Some(shown) = shown else {
        return Err(PageError::not_found(match commit {
            Some(commit) => format!(
                "No snapshot of {} recorded commit {commit}.",
                source.clone_folder
            ),
            None => format!("{} has no snapshot yet.", source.clone_folder),
        }));
    };
    let commit = shown.commit_hash.clone();
    let clone = clone_dir.join(&source.clone_folder);
    let files = git::markdown_files(&clone, &commit, |_, _| ()).map_err(PageError::internal)?;
    Ok(Snapshot {
        source,
        clone,
        revisions,
        commit,
        files,
    })
}

async fn sources_page(State(app): State<App>, Query(flash): Query<Flash>) -> PageResult {
    with_db(&app, move |conn| {
        let sources = db::sources(conn).map_err(internal)?;
        Ok(layout::page("Sources", &flash, page(&sources)))
    })
    .await
}

#[derive(Deserialize)]
struct SourceQuery {
    id: i64,
    commit: Option<String>,
}

async fn source_page(
    State(app): State<App>,
    Query(query): Query<SourceQuery>,
    Query(flash): Query<Flash>,
) -> PageResult {
    let clone_dir = app.clone_dir.clone();
    with_db(&app, move |conn| {
        let snapshot = snapshot(conn, &clone_dir, query.id, query.commit.as_deref())?;
        Ok(layout::page(
            &snapshot.source.clone_folder,
            &flash,
            source(&snapshot),
        ))
    })
    .await
}

fn source(snapshot: &Snapshot) -> Markup {
    let source = &snapshot.source;
    html! {
        p.crumbs { a href="/" { "Sources" } }
        h1 { (source.clone_folder) }
        p.note { (source.url) }
        form.inline method="post" action="/snapshot" {
            input type="hidden" name="url" value=(source.url);
            button type="submit" { "Fetch latest" }
        }

        h2 { "Markdown files at " (layout::commit(&snapshot.commit)) }
        @if snapshot.files.is_empty() {
            p.empty { "No Markdown files at this commit." }
        } @else {
            ul.files {
                @for path in &snapshot.files {
                    li { a href=(files::href(source.id, &snapshot.commit, path)) { (path) } }
                }
            }
            p.note {
                (counted(snapshot.files.len() as u64, "Markdown file", "Markdown files"))
            }
        }

        h2 { "Recorded commits" }
        table {
            thead {
                tr {
                    th scope="col" { "Commit" }
                    th scope="col" { "Recorded" }
                    th scope="col" { "Files" }
                }
            }
            tbody {
                @for revision in &snapshot.revisions {
                    tr {
                        td { (layout::commit(&revision.commit_hash)) }
                        td { (layout::utc(&revision.recorded_at)) }
                        td {
                            @if revision.commit_hash == snapshot.commit {
                                "Shown above"
                            } @else {
                                a href=(href(source.id, Some(&revision.commit_hash))) {
                                    "Show files"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn page(sources: &[SourceSummary]) -> Markup {
    html! {
        h1 { "Sources" }
        p.lead {
            "Git repositories whose docs Distillery distills. Each snapshot records the commit "
            "of the default branch."
        }
        form.inline method="post" action="/snapshot" {
            label for="url" { "Repository URL" }
            input type="text" id="url" name="url" required size="50"
                placeholder="https://github.com/wetfish/server"
                spellcheck="false" autocomplete="off";
            button type="submit" { "Add source" }
        }
        p.note {
            "An https or ssh URL. Distillery clones the repository with its full history and "
            "records the commit of its default branch."
        }
        @if sources.is_empty() {
            p.empty { "No sources yet." }
        } @else {
            table {
                thead {
                    tr {
                        th scope="col" { "Source" }
                        th scope="col" { "Latest commit" }
                        th scope="col" { "Recorded" }
                        th.number scope="col" { "Commits" }
                        th scope="col" { "Snapshot" }
                    }
                }
                tbody {
                    @for source in sources {
                        tr {
                            td {
                                strong {
                                    a href=(href(source.id, None)) { (source.clone_folder) }
                                }
                                br;
                                span.note { (source.url) }
                            }
                            @if let Some((hash, recorded_at)) = &source.latest {
                                td { (layout::commit(hash)) }
                                td { (layout::utc(recorded_at)) }
                            } @else {
                                td.note colspan="2" { "No snapshot yet" }
                            }
                            td.number { (source.revisions) }
                            td {
                                form method="post" action="/snapshot" {
                                    input type="hidden" name="url" value=(source.url);
                                    button type="submit" { "Fetch latest" }
                                }
                            }
                        }
                    }
                }
            }
            p.note { (counted(sources.len() as u64, "source", "sources")) }
        }
    }
}
