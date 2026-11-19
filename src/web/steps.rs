//! The steps every file goes through, shown the same way on every page:
//! snapshot, atomic claims, compound claims, review. Each step says where
//! it stands, and the step that comes next carries the button for it.

use maud::{Markup, html};
use rusqlite::Connection;

use super::{PageError, files, internal, review as review_pages};
use crate::db;
use crate::group;
use crate::progress::counted;
use crate::review;

/// Whether a file's claims are grouped into compound claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grouping {
    /// Too few claims for grouping to make sense.
    NotNeeded,
    Needed,
    Done,
}

/// How far a file has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FileProgress {
    /// Atomic claims extracted.
    pub claims: usize,
    pub grouping: Grouping,
    pub compounds: usize,
    /// Things to review: compound claims and atomic claims.
    pub items: usize,
    /// Items decided or flagged.
    pub handled: usize,
}

impl FileProgress {
    pub fn distilled(&self) -> bool {
        self.claims > 0
    }

    pub fn grouped(&self) -> bool {
        self.distilled() && self.grouping != Grouping::Needed
    }

    pub fn reviewed(&self) -> bool {
        self.grouped() && self.handled == self.items
    }

    /// How much of the way is done, from 0 to 100. Each of the four steps
    /// is a quarter, and review fills its quarter item by item.
    pub fn percent(&self) -> usize {
        let review = if self.grouped() && self.items > 0 {
            25 * self.handled / self.items
        } else {
            0
        };
        25 + 25 * usize::from(self.distilled()) + 25 * usize::from(self.grouped()) + review
    }
}

/// How far the file with these contents has come.
pub(super) fn file_progress(conn: &Connection, blob_hash: &str) -> Result<FileProgress, PageError> {
    let claims = db::claims_of_blob(conn, blob_hash).map_err(internal)?;
    let atomic: Vec<i64> = claims.iter().map(|shown| shown.id).collect();
    // Compound claims count only while their grouping holds exactly the
    // claims shown.
    let compounds = match db::latest_grouping(conn, blob_hash).map_err(internal)? {
        Some(recorded) if recorded.grouping.reason.is_none() => {
            let shown = group::claims_hash(&claims).map_err(PageError::internal)?;
            if shown == recorded.grouping.claims_hash {
                Some(db::compounds(conn, recorded.id).map_err(internal)?)
            } else {
                None
            }
        }
        _ => None,
    };
    let grouping = match &compounds {
        Some(_) => Grouping::Done,
        None if claims.len() < group::FEWEST_CLAIMS => Grouping::NotNeeded,
        None => Grouping::Needed,
    };
    let compounds = compounds.unwrap_or_default();
    let parts: Vec<(i64, &[i64])> = compounds
        .iter()
        .map(|shown| (shown.id, shown.compound.parts.as_slice()))
        .collect();
    let queue = review::queue(&atomic, &parts);
    let ids: Vec<i64> = queue.iter().map(|item| item.claim_id).collect();
    let decisions = db::decisions(conn, &ids).map_err(internal)?;
    Ok(FileProgress {
        claims: claims.len(),
        grouping,
        compounds: compounds.len(),
        items: queue.len(),
        handled: review::handled(&queue, &decisions),
    })
}

/// One step as shown.
struct Step {
    name: &'static str,
    done: bool,
    /// Where the step stands, in a few words.
    status: String,
    /// The step's button or link.
    action: Option<Markup>,
}

/// A file's steps with their buttons. The first step that isn't done is
/// marked as the one to take next.
pub(super) fn file(progress: &FileProgress, source: i64, commit: &str, path: &str) -> Markup {
    // A form with one button, which posts the file to an address.
    let button = |address: &str, words: &str| {
        html! {
            form method="post" action=(address) {
                input type="hidden" name="source" value=(source);
                input type="hidden" name="commit" value=(commit);
                input type="hidden" name="path" value=(path);
                button type="submit" { (words) }
            }
        }
    };
    let review = review_pages::href(source, commit, path);
    let steps = [
        Step {
            name: "Snapshot",
            done: true,
            status: "Read from the repository".to_owned(),
            action: Some(html! { a href=(files::href(source, commit, path)) { "See the file" } }),
        },
        Step {
            name: "Atomic claims",
            done: progress.distilled(),
            status: if progress.distilled() {
                counted(progress.claims as u64, "claim", "claims")
            } else {
                "Not distilled yet".to_owned()
            },
            action: Some(button(
                "/estimate",
                if progress.distilled() {
                    "Distill again"
                } else {
                    "Distill"
                },
            )),
        },
        Step {
            name: "Compound claims",
            done: progress.grouped(),
            status: match progress.grouping {
                _ if !progress.distilled() => "After atomic claims".to_owned(),
                Grouping::Done => counted(
                    progress.compounds as u64,
                    "compound claim",
                    "compound claims",
                ),
                Grouping::NotNeeded => "Not needed for so few claims".to_owned(),
                Grouping::Needed => "Not grouped yet".to_owned(),
            },
            action: (progress.distilled() && progress.grouping == Grouping::Needed)
                .then(|| button("/group", "Group claims")),
        },
        Step {
            name: "Review",
            done: progress.reviewed(),
            status: if progress.grouped() {
                format!("{} of {} reviewed", progress.handled, progress.items)
            } else {
                "After compound claims".to_owned()
            },
            action: progress.grouped().then(|| {
                html! {
                    a.button href=(review) {
                        @if progress.reviewed() {
                            "See the review"
                        } @else if progress.handled > 0 {
                            "Continue review"
                        } @else {
                            "Start review"
                        }
                    }
                }
            }),
        },
    ];
    list(&steps, progress.percent())
}

/// A source's steps: how many of its files have come how far. `files` is
/// how many Markdown files the snapshot has.
pub(super) fn source(files: usize, progress: &[FileProgress]) -> Markup {
    let distilled = progress.iter().filter(|file| file.distilled()).count();
    let grouped = progress.iter().filter(|file| file.grouped()).count();
    let reviewed = progress.iter().filter(|file| file.reviewed()).count();
    let of =
        |done: usize, all: usize| format!("{done} of {}", counted(all as u64, "file", "files"));
    let steps = [
        Step {
            name: "Snapshot",
            done: true,
            status: counted(files as u64, "Markdown file", "Markdown files"),
            action: None,
        },
        Step {
            name: "Atomic claims",
            done: files > 0 && distilled == files,
            status: of(distilled, files),
            action: None,
        },
        Step {
            name: "Compound claims",
            done: files > 0 && grouped == files,
            status: of(grouped, files),
            action: None,
        },
        Step {
            name: "Review",
            done: files > 0 && reviewed == files,
            status: of(reviewed, files),
            action: None,
        },
    ];
    // A file not started yet has only its snapshot.
    let started: usize = progress.iter().map(FileProgress::percent).sum();
    let percent = match files {
        0 => 0,
        _ => (started + 25 * (files - progress.len().min(files))) / files,
    };
    list(&steps, percent)
}

fn list(steps: &[Step], percent: usize) -> Markup {
    let next = steps.iter().position(|step| !step.done);
    html! {
        nav.steps aria-label="Steps" {
            progress max="100" value=(percent) { (percent) "%" }
            ol {
                @for (index, step) in steps.iter().enumerate() {
                    li.done[step.done].next[next == Some(index)] {
                        span.marker {
                            @if step.done { "✓" } @else { (index + 1) }
                        }
                        strong { (step.name) }
                        span.note { (step.status) }
                        @if let Some(action) = &step.action {
                            (action)
                        }
                    }
                }
            }
        }
    }
}
