//! The Review page: one claim of a file at a time, as a question with two
//! answers. The page shows the statement and what it cites, and asks whether
//! it is accurate. Yes saves and moves on. No leads to what is wrong, one
//! choice per button. Details and second thoughts are offered after a
//! decision, never before it. Decisions are recorded under the
//! `review.reviewer` setting.

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::get;
use axum::{Form, Router};
use maud::{Markup, html};
use serde::Deserialize;

use super::layout;
use super::sources::{self as source_pages, Snapshot};
use super::{App, Flash, PageError, PageResult, failed, files, internal, steps, with_db};
use crate::db::{self, RecordedDecision, ShownClaim, ShownCompound};
use crate::git;
use crate::group;
use crate::review::{self, FLAGS, Item, REJECTIONS, Standing, State as Stands, Submitted};

pub(super) fn routes() -> Router<App> {
    Router::new().route("/review", get(review_page).post(decide))
}

/// The address of a file's Review page, which shows the next claim without
/// a decision.
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
    /// The claim to show, whatever its standing.
    claim: Option<i64>,
    /// Show the next claim without a decision after this one.
    after: Option<i64>,
    /// Where the reviewer is with the claim shown: `no` for what is wrong
    /// with it, `reword` or `details`. The question itself without one.
    step: Option<String>,
    /// The claim just decided, which the page offers to add details to.
    last: Option<i64>,
}

/// A file's claims as review sees them: the atomic claims shown on its
/// page, and the compound claims of its grouping when that grouped exactly
/// those claims.
struct Claims {
    snapshot: Snapshot,
    blob_hash: String,
    atomic: Vec<ShownClaim>,
    compounds: Vec<ShownCompound>,
    decisions: Vec<RecordedDecision>,
}

impl Claims {
    /// The claims in the order they are reviewed.
    fn queue(&self) -> Vec<Item> {
        let atomic: Vec<i64> = self.atomic.iter().map(|shown| shown.id).collect();
        let compounds: Vec<(i64, &[i64])> = self
            .compounds
            .iter()
            .map(|shown| (shown.id, shown.compound.parts.as_slice()))
            .collect();
        review::queue(&atomic, &compounds)
    }
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
        blob_hash,
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
        let progress = steps::file_progress(conn, &claims.blob_hash)?;
        let queue = claims.queue();
        // The claim asked for, or the next one without a decision.
        let current = query
            .claim
            .and_then(|claim| queue.iter().find(|item| item.claim_id == claim).copied())
            .or_else(|| review::next_pending(&queue, &claims.decisions, query.after));
        let view = View {
            claims: &claims,
            queue: &queue,
            current,
            step: query.step.as_deref().unwrap_or(""),
            last: query.last,
            path: &query.path,
            reviewer: &reviewer,
            steps: steps::file(
                &progress,
                query.source,
                &claims.snapshot.commit,
                &query.path,
            ),
        };
        Ok(layout::page(
            &format!("Review of {}", query.path),
            &flash,
            page(&view),
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
    /// The button pressed: `confirm`, `split`, `edit`, `details`, or a
    /// reason with its kind, as `reject:out_of_date` or `flag:needs_edit`.
    action: String,
    #[serde(default)]
    text: String,
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
            claim: None,
            after: None,
            step: None,
            last: None,
        };
        let claims = load(conn, &clone_dir, &query)?;
        let page = href(query.source, &claims.snapshot.commit, &query.path);
        // After a decision comes the next claim without one, with the
        // decision just made shown above it. After a refused submission the
        // same claim shows again, with the reason.
        let next = format!("{page}&after={0}&last={0}", form.claim);
        let same = format!("{page}&claim={}", form.claim);

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
        // Buttons carry the reason with the action, as `reject:incorrect`.
        let (action, why) = form
            .action
            .split_once(':')
            .unwrap_or((form.action.as_str(), ""));
        let submitted = Submitted {
            action,
            text: &form.text,
            category: why,
            flag: why,
            reason: &form.reason,
        };
        let latest = review::standing(&claims.decisions, form.claim).latest;
        match review::decide(
            form.claim,
            compound.is_some(),
            &undecided,
            latest.map(|recorded| &recorded.decision),
            &submitted,
            &reviewer,
        ) {
            Ok(decisions) => {
                db::record_decisions(conn, &decisions).map_err(internal)?;
                Ok(Redirect::to(&next))
            }
            Err(error) => Ok(failed(&same, error)),
        }
    })
    .await
}

/// What the page shows.
struct View<'a> {
    claims: &'a Claims,
    /// The claims in the order they are reviewed.
    queue: &'a [Item],
    /// The claim on the page. `None` once every claim is dealt with.
    current: Option<Item>,
    /// Where the reviewer is with it: `no`, `reword`, `details`, or the
    /// question itself.
    step: &'a str,
    /// The claim just decided.
    last: Option<i64>,
    path: &'a str,
    reviewer: &'a str,
    /// The file's steps, as shown on its other pages.
    steps: Markup,
}

impl View<'_> {
    fn stands(&self, id: i64) -> Standing<'_> {
        review::standing(&self.claims.decisions, id)
    }

    fn atomic(&self, id: i64) -> Option<&ShownClaim> {
        self.claims.atomic.iter().find(|shown| shown.id == id)
    }

    fn compound(&self, id: i64) -> Option<&ShownCompound> {
        self.claims.compounds.iter().find(|shown| shown.id == id)
    }

    /// A claim's text as it reads now, after any rewording.
    fn text(&self, id: i64) -> &str {
        let written = match (self.compound(id), self.atomic(id)) {
            (Some(shown), _) => shown.compound.text.as_str(),
            (None, Some(shown)) => shown.claim.text.as_str(),
            (None, None) => "",
        };
        self.stands(id).edited.unwrap_or(written)
    }

    /// The address of the Review page, to add a claim and a step to.
    fn page(&self) -> String {
        let snapshot = &self.claims.snapshot;
        href(snapshot.source.id, &snapshot.commit, self.path)
    }
}

fn page(view: &View) -> Markup {
    let snapshot = &view.claims.snapshot;
    let source = &snapshot.source;
    let handled = review::handled(view.queue, &view.claims.decisions);
    let count = |wanted: fn(&Stands) -> bool| {
        view.claims
            .atomic
            .iter()
            .filter(|shown| wanted(&view.stands(shown.id).state))
            .count()
    };
    html! {
        p.crumbs {
            a href="/" { "Sources" }
            " / "
            a href=(source_pages::href(source.id, Some(&snapshot.commit))) {
                (source.clone_folder)
            }
            " / "
            a href=(files::href(source.id, &snapshot.commit, view.path)) { (view.path) }
        }
        (view.steps)

        div.review-progress {
            h1 { "Review" }
            span.note { (handled) " of " (view.queue.len()) " reviewed" }
        }
        progress.review max=(view.queue.len().max(1)) value=(handled) {
            (handled) " of " (view.queue.len())
        }
        @if let Some(last) = view.last {
            (decided(view, last))
        }

        @if view.queue.is_empty() {
            p.empty { "This file has no claims yet. Distill it first." }
        } @else if let Some(item) = view.current {
            section.review {
                @match view.step {
                    "no" => (wrong(view, item)),
                    "reword" => (reword(view, item)),
                    "details" => (details(view, item)),
                    _ => (question(view, item)),
                }
            }
            @if view.step.is_empty() {
                (turns(view, item))
            }
        } @else {
            section.review {
                h2 { "Every claim of this file is reviewed" }
                p {
                    (count(|state| *state == Stands::Confirmed)) " confirmed, "
                    (count(|state| matches!(state, Stands::Rejected(_)))) " rejected, "
                    (count(|state| matches!(state, Stands::Flagged(_))))
                    " sent back to the model."
                }
                p.answers {
                    a.button href=(files::href(source.id, &snapshot.commit, view.path)) {
                        "Back to the file"
                    }
                    @if let Some(first) = view.queue.first() {
                        a href=(format!("{}&claim={}", view.page(), first.claim_id)) {
                            "Go through the decisions again"
                        }
                    }
                }
            }
        }
        p.note { "Decisions are recorded for " strong { (view.reviewer) } "." }
    }
}

/// The decision just made, with the ways to add to it or change it.
fn decided(view: &View, claim: i64) -> Markup {
    let standing = view.stands(claim);
    let page = view.page();
    html! {
        p.decided {
            (badge(&standing)) " " (view.text(claim))
            span.links {
                a href=(format!("{page}&claim={claim}&step=details")) { "Add details" }
                a href=(format!("{page}&claim={claim}")) { "Change" }
            }
        }
    }
}

/// The statement and what it cites.
fn statement(view: &View, item: Item) -> Markup {
    let id = item.claim_id;
    let file = files::href(
        view.claims.snapshot.source.id,
        &view.claims.snapshot.commit,
        view.path,
    );
    // The source passages, each once: a compound claim's parts often rest
    // on the same sentence.
    let parts: Vec<&ShownClaim> = match view.compound(id) {
        Some(shown) => shown
            .compound
            .parts
            .iter()
            .filter_map(|part| view.atomic(*part))
            .collect(),
        None => view.atomic(id).into_iter().collect(),
    };
    let mut passages: Vec<&ShownClaim> = Vec::new();
    for part in &parts {
        if !passages
            .iter()
            .any(|seen| seen.claim.quote == part.claim.quote)
        {
            passages.push(*part);
        }
    }
    html! {
        p.claim { (view.text(id)) }
        @for passage in &passages {
            blockquote {
                (passage.claim.quote)
                " "
                a.note href=(format!("{file}#L{}", passage.claim.start_line)) {
                    "line " (passage.claim.start_line)
                }
            }
        }
        @if item.compound {
            details {
                summary.note { "The " (parts.len()) " claims it stands for" }
                ul {
                    @for part in &parts {
                        li { (view.text(part.id)) " " (badge(&view.stands(part.id))) }
                    }
                }
            }
        } @else if let Some(whole) = item.part_of {
            p.note { "Part of “" (view.text(whole)) "”" }
        }
    }
}

/// What every button's form carries.
fn fields(view: &View, claim: i64) -> Markup {
    let snapshot = &view.claims.snapshot;
    html! {
        input type="hidden" name="source" value=(snapshot.source.id);
        input type="hidden" name="commit" value=(snapshot.commit);
        input type="hidden" name="path" value=(view.path);
        input type="hidden" name="claim" value=(claim);
    }
}

/// The question: is the statement accurate? Yes saves and moves on. No asks
/// what is wrong.
fn question(view: &View, item: Item) -> Markup {
    let id = item.claim_id;
    let standing = view.stands(id);
    html! {
        @if standing.latest.is_some() {
            p { (badge(&standing)) }
        }
        (statement(view, item))
        h2 { "Is this accurate?" }
        form.answers method="post" action="/review" {
            (fields(view, id))
            button.yes type="submit" name="action" value="confirm" { "Yes" }
            a.button.no href=(format!("{}&claim={id}&step=no", view.page())) { "No" }
        }
    }
}

/// After No: what is wrong, one button per answer.
fn wrong(view: &View, item: Item) -> Markup {
    let id = item.claim_id;
    let page = view.page();
    html! {
        (statement(view, item))
        h2 { "What is wrong with it?" }
        form.options method="post" action="/review" {
            (fields(view, id))
            a.button href=(format!("{page}&claim={id}&step=reword")) {
                "The wording is off" span.note { "Reword it yourself" }
            }
            @if item.compound {
                button type="submit" name="action" value="split" {
                    "These don't belong together"
                    span.note { "Review its claims one by one" }
                }
            }
            @for (value, words) in REJECTIONS {
                button type="submit" name="action" value=(format!("reject:{value}")) {
                    (words)
                    @if item.compound { span.note { "Reject it with its claims" } }
                }
            }
            @if !item.compound {
                @for (value, words) in FLAGS {
                    button type="submit" name="action" value=(format!("flag:{value}")) {
                        (words)
                        span.note { "Send it back to the model" }
                    }
                }
            }
        }
        p { a href=(format!("{page}&claim={id}")) { "← Back to the question" } }
    }
}

/// After "the wording is off": the reviewer's own wording.
fn reword(view: &View, item: Item) -> Markup {
    let id = item.claim_id;
    html! {
        (statement(view, item))
        h2 { "How should it read?" }
        form.answers method="post" action="/review" {
            (fields(view, id))
            input type="text" name="text" value=(view.text(id)) aria-label="The claim, reworded";
            button.yes type="submit" name="action" value="edit" { "Save" }
        }
        p { a href=(format!("{}&claim={id}&step=no", view.page())) { "← Back" } }
    }
}

/// After a decision: the reviewer's own words about it.
fn details(view: &View, item: Item) -> Markup {
    let id = item.claim_id;
    let standing = view.stands(id);
    html! {
        p { (badge(&standing)) }
        (statement(view, item))
        @if standing.latest.is_some() {
            h2 { "Anything to add?" }
            form.answers method="post" action="/review" {
                (fields(view, id))
                input type="text" name="reason" aria-label="Details";
                button.yes type="submit" name="action" value="details" { "Save" }
            }
        }
        p { a href=(format!("{}&after={id}", view.page())) { "Continue without details →" } }
    }
}

/// The way to the claims before and after the one shown, without deciding.
fn turns(view: &View, item: Item) -> Markup {
    let page = view.page();
    let position = view
        .queue
        .iter()
        .position(|other| other.claim_id == item.claim_id)
        .unwrap_or(0);
    html! {
        p.turns {
            @if let Some(before) = position.checked_sub(1).and_then(|at| view.queue.get(at)) {
                a href=(format!("{page}&claim={}", before.claim_id)) { "← Previous" }
            } @else {
                span.note { "← Previous" }
            }
            span.note { (position + 1) " of " (view.queue.len()) }
            a href=(format!("{page}&after={}", item.claim_id)) { "Skip →" }
        }
    }
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
