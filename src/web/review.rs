//! The Review page: a file's claims, compound claims first with their parts,
//! each beside its source text and with the decisions a reviewer can make.
//! Decisions are recorded under the `review.reviewer` setting.

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::get;
use axum::{Form, Router};
use maud::{Markup, html};
use serde::Deserialize;

use super::layout;
use super::sources::{self as source_pages, Snapshot};
use super::{App, Flash, PageError, PageResult, failed, files, internal, with_db};
use crate::db::{self, RecordedDecision, ShownClaim, ShownCompound};
use crate::git;
use crate::group;
use crate::progress::counted;
use crate::review::{self, FLAGS, REJECTIONS, Standing, State as Stands, Submitted};

pub(super) fn routes() -> Router<App> {
    Router::new().route("/review", get(review_page).post(decide))
}

/// The address of a file's Review page.
pub(super) fn href(source: i64, commit: &str, path: &str) -> String {
    format!(
        "/review?source={source}&commit={commit}&path={}",
        layout::url_encode(path)
    )
}

#[derive(Deserialize)]
struct ReviewQuery {
    source: i64,
    commit: String,
    path: String,
}

/// A file's claims as review sees them: the atomic claims shown on its
/// page, and the compound claims of its grouping when that grouped exactly
/// those claims.
struct Claims {
    snapshot: Snapshot,
    atomic: Vec<ShownClaim>,
    compounds: Vec<ShownCompound>,
    decisions: Vec<RecordedDecision>,
}

fn load(
    conn: &rusqlite::Connection,
    clone_dir: &std::path::Path,
    query: &ReviewQuery,
) -> Result<Claims, PageError> {
    let snapshot = source_pages::snapshot(conn, clone_dir, query.source, Some(&query.commit))?;
    // Only the Markdown files listed at the commit can be read.
    if !snapshot.files.contains(&query.path) {
        return Err(PageError::not_found(format!(
            "{} is not a Markdown file at commit {}.",
            query.path, snapshot.commit
        )));
    }
    let blob_hash = git::blob_at(&snapshot.clone, &snapshot.commit, &query.path)
        .map_err(PageError::internal)?;
    let atomic = db::claims_of_blob(conn, &blob_hash).map_err(internal)?;
    // Compound claims count only while their grouping holds exactly the
    // claims shown.
    let compounds = match db::latest_grouping(conn, &blob_hash).map_err(internal)? {
        Some(recorded) if recorded.grouping.reason.is_none() => {
            let shown = group::claims_hash(&atomic).map_err(PageError::internal)?;
            if shown == recorded.grouping.claims_hash {
                db::compounds(conn, recorded.id).map_err(internal)?
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    };
    let ids: Vec<i64> = atomic
        .iter()
        .map(|shown| shown.id)
        .chain(compounds.iter().map(|shown| shown.id))
        .collect();
    let decisions = db::decisions(conn, &ids).map_err(internal)?;
    Ok(Claims {
        snapshot,
        atomic,
        compounds,
        decisions,
    })
}

async fn review_page(
    State(app): State<App>,
    Query(query): Query<ReviewQuery>,
    Query(flash): Query<Flash>,
) -> PageResult {
    let clone_dir = app.clone_dir.clone();
    let reviewer = app.reviewer.clone();
    with_db(&app, move |conn| {
        let claims = load(conn, &clone_dir, &query)?;
        Ok(layout::page(
            &format!("Review of {}", query.path),
            &flash,
            page(&claims, &query.path, &reviewer),
        ))
    })
    .await
}

#[derive(Deserialize)]
struct DecideForm {
    source: i64,
    commit: String,
    path: String,
    claim: i64,
    /// `confirm`, `edit`, `reject` or `flag`: the button pressed.
    action: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    flag: String,
    #[serde(default)]
    reason: String,
}

/// Records a reviewer's decision about one claim of a file, and returns to
/// the claim on the Review page.
async fn decide(
    State(app): State<App>,
    Form(form): Form<DecideForm>,
) -> Result<Redirect, PageError> {
    let clone_dir = app.clone_dir.clone();
    let reviewer = app.reviewer.clone();
    with_db(&app, move |conn| {
        let query = ReviewQuery {
            source: form.source,
            commit: form.commit,
            path: form.path,
        };
        let claims = load(conn, &clone_dir, &query)?;
        let page = href(query.source, &claims.snapshot.commit, &query.path);
        let back = format!("{page}#C{}", form.claim);

        // Decisions are only taken for the claims the page shows.
        let compound = claims.compounds.iter().find(|shown| shown.id == form.claim);
        if compound.is_none() && !claims.atomic.iter().any(|shown| shown.id == form.claim) {
            return Ok(failed(&page, "This claim is no longer shown for the file."));
        }
        let undecided: Vec<i64> = compound
            .map(|shown| shown.compound.parts.as_slice())
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|part| review::standing(&claims.decisions, *part).state == Stands::Proposed)
            .collect();
        let submitted = Submitted {
            action: &form.action,
            text: &form.text,
            category: &form.category,
            flag: &form.flag,
            reason: &form.reason,
        };
        match review::decide(
            form.claim,
            compound.is_some(),
            &undecided,
            &submitted,
            &reviewer,
        ) {
            Ok(decisions) => {
                db::record_decisions(conn, &decisions).map_err(internal)?;
                Ok(Redirect::to(&back))
            }
            // The page shows the error at its top, so the claim's place in
            // the address is left out.
            Err(error) => Ok(failed(&page, error)),
        }
    })
    .await
}

fn page(claims: &Claims, path: &str, reviewer: &str) -> Markup {
    let snapshot = &claims.snapshot;
    let source = &snapshot.source;
    let stands = |id: i64| review::standing(&claims.decisions, id);
    let decided = claims
        .atomic
        .iter()
        .filter(|shown| stands(shown.id).state.decided())
        .count();
    let flagged = claims
        .atomic
        .iter()
        .filter(|shown| matches!(stands(shown.id).state, Stands::Flagged(_)))
        .count();
    let grouped: Vec<i64> = claims
        .compounds
        .iter()
        .flat_map(|shown| shown.compound.parts.iter().copied())
        .collect();
    let ungrouped: Vec<&ShownClaim> = claims
        .atomic
        .iter()
        .filter(|shown| !grouped.contains(&shown.id))
        .collect();
    let form = Fields {
        source: source.id,
        commit: &snapshot.commit,
        path,
    };
    html! {
        p.crumbs {
            a href="/" { "Sources" }
            " / "
            a href=(source_pages::href(source.id, Some(&snapshot.commit))) {
                (source.clone_folder)
            }
            " / "
            a href=(files::href(source.id, &snapshot.commit, path)) { (path) }
        }
        h1 { "Review" }
        p.lead {
            (decided) " of " (counted(claims.atomic.len() as u64, "claim", "claims"))
            " decided"
            @if flagged > 0 { ", " (flagged) " flagged" }
            ". Decisions are recorded for " strong { (reviewer) } " and never changed: a "
            "later decision replaces an earlier one."
        }
        p.note {
            "Confirming or rejecting a compound claim decides its undecided parts the same "
            "way. Edits confirm a claim with new text. Flagged notes wait for another pass by "
            "the model."
        }

        @if claims.atomic.is_empty() {
            p.empty { "This file has no claims yet. Distill it first." }
        }
        @for shown in &claims.compounds {
            section.review id=(format!("C{}", shown.id)) {
                p {
                    (badge(&stands(shown.id)))
                    " " span.kind { (shown.compound.kind) } " "
                    strong { (stands(shown.id).edited.unwrap_or(&shown.compound.text)) }
                }
                (decision_form(&form, shown.id, stands(shown.id).edited.unwrap_or(&shown.compound.text), true))
                ul {
                    @for part in &shown.compound.parts {
                        @if let Some(part) = claims.atomic.iter().find(|atomic| atomic.id == *part) {
                            (atomic(&form, part, &stands(part.id)))
                        }
                    }
                }
            }
        }
        @if !ungrouped.is_empty() {
            section.review {
                p {
                    strong {
                        @if claims.compounds.is_empty() { "Claims" } @else { "Claims outside compound claims" }
                    }
                }
                ul {
                    @for shown in &ungrouped {
                        (atomic(&form, shown, &stands(shown.id)))
                    }
                }
            }
        }
    }
}

/// What every decision form on a page carries.
struct Fields<'a> {
    source: i64,
    commit: &'a str,
    path: &'a str,
}

/// Where a claim stands, as a label with who decided and why.
fn badge(standing: &Standing) -> Markup {
    html! {
        @match &standing.state {
            Stands::Proposed => span.kind { "AI proposal" },
            Stands::Confirmed => span.kind.confirmed {
                @if standing.edited.is_some() { "Confirmed, edited" } @else { "Confirmed" }
            },
            Stands::Rejected(category) => span.kind.rejected {
                "Rejected: " (review::label(category))
            },
            Stands::Flagged(category) => span.kind.flagged {
                "Flagged: " (review::label(category))
            },
        }
        @if let Some(latest) = standing.latest {
            @if let Some(reason) = &latest.decision.reason {
                " " span.note { "“" (reason) "”" }
            }
            " "
            span.note title=(format!("{} UTC", latest.created_at)) {
                "by " (latest.decision.reviewer)
            }
        }
    }
}

/// One atomic claim: its note, its source text and its decision form.
fn atomic(form: &Fields, shown: &ShownClaim, standing: &Standing) -> Markup {
    let claim = &shown.claim;
    let text = standing.edited.unwrap_or(&claim.text);
    html! {
        li id=(format!("C{}", shown.id)) {
            (badge(standing))
            " " span.kind { (claim.kind) }
            @if claim.illustrative {
                " " span.kind { "illustrative" }
            }
            " " (text)
            br;
            span.note {
                a href=(format!(
                    "{}#L{}",
                    files::href(form.source, form.commit, form.path),
                    claim.start_line
                )) {
                    @if claim.start_line == claim.end_line {
                        "line " (claim.start_line)
                    } @else {
                        "lines " (claim.start_line) "–" (claim.end_line)
                    }
                }
                " · " q { (claim.quote) }
            }
            (decision_form(form, shown.id, text, false))
        }
    }
}

/// The decisions a reviewer can make about one claim. Compound claims take
/// no flags: their parts do.
fn decision_form(form: &Fields, claim: i64, text: &str, compound: bool) -> Markup {
    html! {
        form.review method="post" action="/review" {
            input type="hidden" name="source" value=(form.source);
            input type="hidden" name="commit" value=(form.commit);
            input type="hidden" name="path" value=(form.path);
            input type="hidden" name="claim" value=(claim);
            button type="submit" name="action" value="confirm" {
                @if compound { "Confirm with parts" } @else { "Confirm" }
            }
            input type="text" name="text" value=(text) aria-label="Claim text" size="40";
            button type="submit" name="action" value="edit" { "Save edit" }
            select name="category" aria-label="Why the claim is rejected" {
                option value="" { "Reject as…" }
                @for (value, words) in REJECTIONS {
                    option value=(value) { (words) }
                }
            }
            button type="submit" name="action" value="reject" {
                @if compound { "Reject with parts" } @else { "Reject" }
            }
            @if !compound {
                select name="flag" aria-label="Why the note is flagged" {
                    option value="" { "Flag as…" }
                    @for (value, words) in FLAGS {
                        option value=(value) { (words) }
                    }
                }
                button type="submit" name="action" value="flag" { "Flag" }
            }
            input type="text" name="reason" placeholder="Reason, optional" aria-label="Reason" size="24";
        }
    }
}
