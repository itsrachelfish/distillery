//! The Usage page: every model API call with its tokens and cost, added up
//! by model, operation and day, and a check that the API key works.

use std::path::Path;
use std::thread;
use std::time::Instant;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use maud::{Markup, html};

use super::layout::{self, dollars, number};
use super::{App, Flash, PageResult, done, failed, internal, with_db};
use crate::anthropic::Anthropic;
use crate::config;
use crate::db::{self, ModelCall, RecordedCall, UsageGroup, UsageTotal};
use crate::model::{ModelProvider, Prompt};

/// How many of the latest calls the page lists.
const LATEST_CALLS: i64 = 50;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/usage", get(usage_page))
        .route("/usage/check", post(check_key))
}

/// What the page shows.
struct Usage {
    by_model: Vec<UsageTotal>,
    by_operation: Vec<UsageTotal>,
    by_day: Vec<UsageTotal>,
    latest: Vec<RecordedCall>,
}

async fn usage_page(State(app): State<App>, Query(flash): Query<Flash>) -> PageResult {
    let model = app.model.clone();
    with_db(&app, move |conn| {
        let usage = Usage {
            by_model: db::usage_totals(conn, UsageGroup::Model).map_err(internal)?,
            by_operation: db::usage_totals(conn, UsageGroup::Operation).map_err(internal)?,
            by_day: db::usage_totals(conn, UsageGroup::Day).map_err(internal)?,
            latest: db::latest_model_calls(conn, LATEST_CALLS).map_err(internal)?,
        };
        Ok(layout::page("Usage", &flash, page(&model, &usage)))
    })
    .await
}

/// Checks the API key with a token count, which the API does for free, and
/// records the call.
async fn check_key(State(app): State<App>) -> Redirect {
    // The HTTPS client blocks, so it runs on a thread of its own, away from
    // the threads that serve pages.
    let checked = tokio::task::spawn_blocking(move || {
        thread::spawn(move || run_check(&app.db, &app.model)).join()
    })
    .await;
    match checked {
        Ok(Ok(Ok(tokens))) => done(
            "/usage",
            &format!("The API key works: the API counted {tokens} tokens, at no cost."),
        ),
        Ok(Ok(Err(err))) => failed("/usage", err),
        _ => failed(
            "/usage",
            "The check stopped unexpectedly; see the terminal.",
        ),
    }
}

fn run_check(database: &Path, model: &config::Model) -> Result<u64, String> {
    // Without a key no call is made, so none is recorded.
    let api = Anthropic::new(model)?;
    let started = Instant::now();
    let counted = api.count_tokens(&Prompt {
        system: None,
        user: "ping".to_owned(),
        schema: None,
    });
    let call = ModelCall {
        provider: api.provider().to_owned(),
        model: api.model().to_owned(),
        operation: "check_key".to_owned(),
        // Counting is free and sends nothing to a model, so no tokens are
        // used.
        input_tokens: 0,
        output_tokens: 0,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        cost_micros: 0,
        duration_ms: started.elapsed().as_millis() as i64,
        stop_reason: None,
        error: counted.as_ref().err().map(|error| error.message.clone()),
        http_status: counted
            .as_ref()
            .err()
            .and_then(|error| error.status)
            .map(i64::from),
    };
    let conn = db::open(database).map_err(|err| format!("Opening the database: {err}"))?;
    db::record_model_call(&conn, &call).map_err(|err| format!("Recording the call: {err}"))?;
    counted.map_err(|error| error.message)
}

fn page(model: &config::Model, usage: &Usage) -> Markup {
    html! {
        h1 { "Usage" }
        p.lead {
            "Every call to the model API is recorded here with its tokens and cost, failed "
            "calls included. Rate-limited calls are failed calls the API answered with 429."
        }

        h2 { "Settings in use" }
        table {
            tbody {
                tr {
                    th scope="row" { "Model" }
                    td { code { (model.name) } }
                }
                tr {
                    th scope="row" { "Input price" }
                    td { (price(model.input_price)) " per million tokens" }
                }
                tr {
                    th scope="row" { "Output price" }
                    td { (price(model.output_price)) " per million tokens" }
                }
                tr {
                    th scope="row" { "Run cost limit" }
                    td { (price(model.max_run_cost)) " per run" }
                }
                tr {
                    th scope="row" { "API key" }
                    td {
                        @if model.api_key.is_empty() {
                            "Not set. Add " code { "api_key" } " under " code { "[model]" }
                            " in " code { "distillery.toml" } " and restart the server."
                        } @else {
                            "Set"
                        }
                    }
                }
            }
        }
        p.note {
            "These come from the " code { "[model]" } " settings, read when the server "
            "starts. Costs are computed from the prices at the time of each call."
        }
        form.inline method="post" action="/usage/check" {
            button type="submit" { "Check API key" }
            span.note { "Asks the API to count the tokens of one word, which is free." }
        }

        @if usage.latest.is_empty() {
            p.empty { "No calls recorded yet." }
        } @else {
            h2 { "By model" }
            (totals("Model", &usage.by_model))
            h2 { "By operation" }
            (totals("Operation", &usage.by_operation))
            h2 { "By day" }
            (totals("Day (UTC)", &usage.by_day))
            h2 { "Latest calls" }
            (latest(&usage.latest))
        }
    }
}

/// A configured price in US dollars.
fn price(dollars: f64) -> String {
    format!("${dollars:.2}")
}

fn totals(group: &str, rows: &[UsageTotal]) -> Markup {
    html! {
        table {
            thead {
                tr {
                    th scope="col" { (group) }
                    th.number scope="col" { "Calls" }
                    th.number scope="col" { "Failed" }
                    th.number scope="col" { "Rate limited" }
                    th.number scope="col" { "Tokens in" }
                    th.number scope="col" { "Tokens out" }
                    th.number scope="col" { "Cost" }
                }
            }
            tbody {
                @for row in rows {
                    tr {
                        td { (row.group) }
                        td.number { (number(row.calls)) }
                        td.number { (number(row.failed)) }
                        td.number { (number(row.rate_limited)) }
                        td.number { (number(row.input_tokens)) }
                        td.number { (number(row.output_tokens)) }
                        td.number { (dollars(row.cost_micros)) }
                    }
                }
            }
        }
    }
}

fn latest(rows: &[RecordedCall]) -> Markup {
    html! {
        table {
            caption.note { "The latest " (LATEST_CALLS) " calls, newest first." }
            thead {
                tr {
                    th scope="col" { "Time" }
                    th scope="col" { "Operation" }
                    th scope="col" { "Model" }
                    th.number scope="col" { "Tokens in" }
                    th.number scope="col" { "Tokens out" }
                    th.number scope="col" { "Cost" }
                    th.number scope="col" { "Took" }
                    th scope="col" { "Result" }
                }
            }
            tbody {
                @for row in rows {
                    @let call = &row.call;
                    tr {
                        td { (layout::utc(&row.created_at)) }
                        td { (call.operation) }
                        td { (call.model) br; span.note { (call.provider) } }
                        td.number title=(format!(
                            "Prompt cache: {} written, {} read",
                            number(call.cache_write_tokens),
                            number(call.cache_read_tokens)
                        )) {
                            (number(call.input_tokens))
                        }
                        td.number { (number(call.output_tokens)) }
                        td.number { (dollars(call.cost_micros)) }
                        td.number { (number(call.duration_ms)) " ms" }
                        td {
                            @match (&call.error, &call.stop_reason) {
                                (Some(error), _) => span.failed { "Failed: " (error) },
                                (None, Some(reason)) => { "OK, " (reason) },
                                (None, None) => "OK",
                            }
                        }
                    }
                }
            }
        }
    }
}
