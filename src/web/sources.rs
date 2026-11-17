//! The Sources page: every repository added, with its latest snapshot.

use axum::Router;
use axum::extract::{Query, State};
use axum::routing::get;
use maud::{Markup, html};

use super::{App, Flash, PageResult, internal, layout, with_db};
use crate::db::{self, SourceSummary};
use crate::progress::counted;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/", get(sources_page))
}

async fn sources_page(State(app): State<App>, Query(flash): Query<Flash>) -> PageResult {
    with_db(&app, move |conn| {
        let sources = db::sources(conn).map_err(internal)?;
        Ok(layout::page("Sources", &flash, page(&sources)))
    })
    .await
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
                                strong { (source.clone_folder) }
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
