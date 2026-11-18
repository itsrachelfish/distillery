//! `distillery serve`: the web interface, served on this computer only.
//!
//! Pages are built on the server with maud and use plain HTML forms, with no
//! JavaScript. A form posts to an action, which redirects back to a page
//! with a notice or an error in the address, so reloading a page never
//! repeats an action. Each request opens the database on a blocking thread,
//! since SQLite calls block.
//!
//! The MVP has no login, so every request has to come from this computer's
//! own pages: see `guard`.

mod estimates;
mod files;
mod jobs;
mod layout;
mod sources;
mod usage;

use std::fmt::Display;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{Markup, html};
use rusqlite::Connection;
use serde::Deserialize;

use crate::config::{self, Config};
use crate::db;

const STYLE: &str = include_str!("style.css");

#[derive(Clone)]
struct App {
    db: Arc<PathBuf>,
    clone_dir: Arc<PathBuf>,
    model: Arc<config::Model>,
    port: u16,
}

/// Serves the pages on 127.0.0.1 until the process is stopped.
pub fn run(config: &Config) -> db::Result<()> {
    let port = config.server.port;
    let app = App {
        db: Arc::new(config.storage.database.clone()),
        clone_dir: Arc::new(config.storage.clone_dir.clone()),
        model: Arc::new(config.model.clone()),
        port,
    };
    let router = Router::new()
        .merge(sources::routes())
        .merge(jobs::routes())
        .merge(files::routes())
        .merge(estimates::routes())
        .merge(usage::routes())
        .route("/style.css", get(style))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app);

    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(address)
            .await
            .map_err(|err| format!("listening on {address}: {err}"))?;
        eprintln!("Serving Distillery at http://{address}/ (press Ctrl+C to stop)");
        axum::serve(listener, router).await?;
        Ok(())
    })
}

/// Rejects requests from anywhere but this computer's own pages.
///
/// Listening on 127.0.0.1 keeps other computers out, but any website open
/// in a browser on this computer can still send requests here. Two checks
/// stop that:
///
/// - `Host` must name this server. Websites that point their own name at
///   127.0.0.1 to read pages arrive with their name in `Host`.
/// - Requests that change something must carry an `Origin` naming this
///   server. Browsers add `Origin` to form posts, so forms on other
///   websites can't record decisions under the reviewer's name.
async fn guard(State(app): State<App>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            request
                .uri()
                .authority()
                .map(|authority| authority.as_str())
        });
    if !host.is_some_and(|host| is_local_host(host, app.port)) {
        return forbidden("Distillery only answers requests addressed to this computer.");
    }

    let reads = [Method::GET, Method::HEAD];
    if !reads.contains(request.method()) {
        let origin = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok());
        if !origin.is_some_and(|origin| is_local_origin(origin, app.port)) {
            return forbidden("Distillery only accepts forms sent from its own pages.");
        }
    }

    next.run(request).await
}

/// Whether a `Host` header names this server: 127.0.0.1 or localhost, with
/// the server's port.
fn is_local_host(host: &str, port: u16) -> bool {
    let (name, given_port) = match host.rsplit_once(':') {
        Some((name, given)) => (name, given.parse::<u16>().ok()),
        // Browsers leave the port out only for port 80.
        None => (host, Some(80)),
    };
    given_port == Some(port) && (name == "127.0.0.1" || name.eq_ignore_ascii_case("localhost"))
}

/// Whether an `Origin` header names this server's pages.
fn is_local_origin(origin: &str, port: u16) -> bool {
    origin
        .strip_prefix("http://")
        .is_some_and(|host| is_local_host(host, port))
}

fn forbidden(message: &'static str) -> Response {
    (StatusCode::FORBIDDEN, message).into_response()
}

/// An error page with an HTTP status.
struct PageError {
    status: StatusCode,
    message: String,
}

impl PageError {
    fn not_found(message: impl Into<String>) -> Self {
        PageError {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        PageError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for PageError {
    fn into_response(self) -> Response {
        let title = if self.status == StatusCode::NOT_FOUND {
            "Not found"
        } else {
            "Error"
        };
        let body = html! {
            h1 { (title) }
            p.flash.error role="alert" { (self.message) }
            p { a href="/" { "Back to sources" } }
        };
        (self.status, layout::page(title, &Flash::default(), body)).into_response()
    }
}

type PageResult = Result<Markup, PageError>;

/// Runs database work on a blocking thread with its own connection.
async fn with_db<T, F>(app: &App, work: F) -> Result<T, PageError>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> Result<T, PageError> + Send + 'static,
{
    let path = Arc::clone(&app.db);
    tokio::task::spawn_blocking(move || {
        let mut conn = db::open(&path)
            .map_err(|err| PageError::internal(format!("opening the database: {err}")))?;
        work(&mut conn)
    })
    .await
    .map_err(|err| PageError::internal(format!("the page failed to build: {err}")))?
}

/// Turns a database error into an error page.
fn internal(err: impl Display) -> PageError {
    PageError::internal(format!("reading the database: {err}"))
}

/// A notice or error carried in the address after an action, shown at the
/// top of the page it redirected to.
#[derive(Default, Deserialize)]
struct Flash {
    notice: Option<String>,
    error: Option<String>,
}

/// Redirects to `back` with a notice.
fn done(back: &str, notice: &str) -> Redirect {
    flash(back, "notice", notice)
}

/// Redirects to `back` with an error.
fn failed(back: &str, error: impl Display) -> Redirect {
    flash(back, "error", &error.to_string())
}

fn flash(back: &str, key: &str, message: &str) -> Redirect {
    // Pages such as a file's already have a query in their address.
    let join = if back.contains('?') { '&' } else { '?' };
    Redirect::to(&format!(
        "{back}{join}{key}={}",
        layout::url_encode(message)
    ))
}

async fn style() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], STYLE)
}

async fn not_found() -> PageError {
    PageError::not_found("There is no page here.")
}
