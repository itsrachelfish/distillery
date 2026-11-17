//! The Sources page: every repository added, with its latest snapshot.

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use maud::{Markup, html};

use super::{App, PageResult, internal, layout, with_db};
use crate::db::{self, SourceSummary};
use crate::progress::counted;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/", get(sources_page))
}

async fn sources_page(State(app): State<App>) -> PageResult {
    with_db(&app, |conn| {
        let sources = db::sources(conn).map_err(internal)?;
        Ok(layout::page("Sources", page(&sources)))
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
        @if sources.is_empty() {
            p.empty {
                "No sources yet. Add one from the command line: "
                code { "distillery snapshot https://github.com/wetfish/server" }
            }
        } @else {
            table {
                thead {
                    tr {
                        th scope="col" { "Source" }
                        th scope="col" { "Latest commit" }
                        th scope="col" { "Recorded" }
                        th.number scope="col" { "Commits" }
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
                        }
                    }
                }
            }
            p.note { (counted(sources.len() as u64, "source", "sources")) }
        }
    }
}
