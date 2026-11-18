//! Grouping: "Group claims" sends the claims of files to the model as a job,
//! one call per file. Nothing else starts these calls.

use axum::extract::State;
use axum::response::Redirect;
use axum::routing::post;
use axum::{Form, Router};

use super::jobs::{self, Finished};
use super::layout::dollars;
use super::sources as source_pages;
use super::{App, PageError, failed, files, with_db};
use crate::anthropic::{self, Anthropic};
use crate::db;
use crate::git;
use crate::group;
use crate::progress::counted;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/group", post(start_group))
}

/// Starts grouping the claims of the named files in the background, unless
/// a job is running, and goes to the Job page.
///
/// The form holds `source`, `commit`, and one `path` per file.
async fn start_group(
    State(app): State<App>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Redirect, PageError> {
    let mut source = None;
    let mut commit = None;
    let mut paths = Vec::new();
    for (name, value) in fields {
        match name.as_str() {
            "source" => source = value.parse::<i64>().ok(),
            "commit" => commit = Some(value),
            "path" => paths.push(value),
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
    if paths.is_empty() {
        return Ok(failed(&back, "The form named no files."));
    }
    // A missing key is refused before a job starts, so the page says why.
    if app.model.api_key.is_empty() {
        return Ok(failed(&back, anthropic::NO_KEY));
    }

    let clone_dir = app.clone_dir.clone();
    let found = with_db(&app, move |conn| {
        let snapshot = source_pages::snapshot(conn, &clone_dir, source, Some(&commit))?;
        // Only the Markdown files listed at the commit can be read.
        if let Some(path) = paths.iter().find(|path| !snapshot.files.contains(path)) {
            return Err(PageError::not_found(format!(
                "{path} is not a Markdown file at commit {}.",
                snapshot.commit
            )));
        }
        Ok((snapshot, paths))
    })
    .await?;
    let (snapshot, paths) = found;

    let title = match paths.as_slice() {
        [path] => format!(
            "Grouping claims of {path} in {}",
            snapshot.source.clone_folder
        ),
        _ => format!(
            "Grouping claims of {} in {}",
            counted(paths.len() as u64, "file", "files"),
            snapshot.source.clone_folder
        ),
    };
    let started = jobs::start(title, move |progress| {
        let mut conn = db::open(&app.db).map_err(|err| format!("Opening the database: {err}"))?;
        let mut blobs = Vec::with_capacity(paths.len());
        for path in &paths {
            blobs.push(git::blob_at(&snapshot.clone, &snapshot.commit, path)?);
        }
        let files: Vec<group::File> = paths
            .iter()
            .zip(&blobs)
            .map(|(path, blob_hash)| group::File {
                path: path.as_str(),
                blob_hash: blob_hash.as_str(),
            })
            .collect();
        // The HTTPS client blocks, so it lives on the job's thread.
        let api = Anthropic::new(&app.model)?;
        let done = group::run(&mut conn, &api, &app.model, &files, progress)?;
        Ok(Finished {
            said: format!(
                "{} and {} from {} for {}. Not grouped: {}. Grouped before: {}. Too few \
                 claims: {}. Not sent: {}.",
                counted(done.compounds as u64, "compound claim", "compound claims"),
                counted(done.links as u64, "relation", "relations"),
                counted(done.grouped as u64, "file", "files"),
                dollars(done.cost_micros),
                done.failed,
                done.cached,
                done.skipped,
                done.unsent
            ),
            link: Some(("See the result".to_owned(), back)),
        })
    });
    Ok(if started {
        Redirect::to("/job")
    } else {
        failed("/job", jobs::BUSY)
    })
}
