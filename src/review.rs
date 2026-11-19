//! Review: what reviewers decide about claims, and where each claim stands.
//!
//! Decisions are recorded in order and never changed. A claim stands where
//! its latest decision put it, and reads as its latest edit wrote it.

use crate::db::{Decision, RecordedDecision};

/// Why claims are rejected: the value recorded, and the words shown.
/// Incorrect and out-of-date claims stay known, so the same statement
/// elsewhere can be flagged. Needless detail and duplicates are dropped.
pub const REJECTIONS: [(&str, &str); 4] = [
    ("incorrect", "Incorrect"),
    ("out_of_date", "Out of date"),
    ("needless", "Needless detail"),
    ("duplicate", "Duplicate"),
];

/// Why notes are flagged for another pass by the model.
pub const FLAGS: [(&str, &str); 2] = [
    ("needs_edit", "Needs editing"),
    ("needs_source", "Unclear without its source"),
];

/// Why a compound claim is rejected on its own, leaving its parts to be
/// reviewed one by one.
pub const SPLIT: (&str, &str) = ("grouping", "Parts don't belong together");

/// The words shown for a rejection or flag category.
pub fn label(category: &str) -> &str {
    REJECTIONS
        .iter()
        .chain(&FLAGS)
        .chain([&SPLIT])
        .find(|(value, _)| *value == category)
        .map_or(category, |(_, words)| words)
}

/// Where a claim stands in review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// No decision yet: an AI proposal.
    Proposed,
    Confirmed,
    /// With the category of the rejection.
    Rejected(String),
    /// Waiting for another pass by the model, with the category of the flag.
    Flagged(String),
}

/// A claim's state, its text if a reviewer edited it, and its latest
/// decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing<'a> {
    pub state: State,
    pub edited: Option<&'a str>,
    pub latest: Option<&'a RecordedDecision>,
}

/// Where a claim stands after `decisions`, which are in the order made and
/// may hold other claims' decisions too.
pub fn standing(decisions: &[RecordedDecision], claim_id: i64) -> Standing<'_> {
    let own = || {
        decisions
            .iter()
            .filter(move |recorded| recorded.decision.claim_id == claim_id)
    };
    let latest = own().last();
    let category =
        |recorded: &RecordedDecision| recorded.decision.category.clone().unwrap_or_default();
    let state = match latest {
        None => State::Proposed,
        Some(recorded) => match recorded.decision.decision.as_str() {
            "rejected" => State::Rejected(category(recorded)),
            "flagged" => State::Flagged(category(recorded)),
            // Edits confirm the claim as edited.
            _ => State::Confirmed,
        },
    };
    let edited = own()
        .filter(|recorded| recorded.decision.decision == "edited")
        .last()
        .and_then(|recorded| recorded.decision.text.as_deref());
    Standing {
        state,
        edited,
        latest,
    }
}

/// What a reviewer submitted for one claim.
pub struct Submitted<'a> {
    /// `confirm`, `reject`, `split`, `flag`, `edit` or `details`.
    pub action: &'a str,
    /// The new text, for edits.
    pub text: &'a str,
    /// Why, for rejections.
    pub category: &'a str,
    /// Why, for flags.
    pub flag: &'a str,
    /// The reviewer's own words, for details.
    pub reason: &'a str,
}

/// The decisions to record for a submission.
///
/// Confirming or rejecting a compound claim decides its `undecided_parts`
/// the same way, so a group is reviewed in one step; parts decided on their
/// own keep their decisions. Splitting rejects a compound claim alone, and
/// its parts stay to be reviewed. Atomic claims have no parts. Details
/// repeat the claim's `latest` decision with the reviewer's words added.
pub fn decide(
    claim_id: i64,
    compound: bool,
    undecided_parts: &[i64],
    latest: Option<&Decision>,
    submitted: &Submitted,
    reviewer: &str,
) -> Result<Vec<Decision>, String> {
    let decision =
        |claim_id, decision: &str, category: Option<&str>, text: Option<&str>| Decision {
            claim_id,
            decision: decision.to_owned(),
            category: category.map(str::to_owned),
            text: text.map(str::to_owned),
            reason: None,
            reviewer: reviewer.to_owned(),
        };
    // The claim itself, then the parts decided along with it.
    let with_parts = |name: &str, category: Option<&str>| {
        std::iter::once(claim_id)
            .chain(undecided_parts.iter().copied())
            .map(|id| decision(id, name, category, None))
            .collect()
    };
    match submitted.action {
        "confirm" => Ok(with_parts("confirmed", None)),
        "reject" => {
            if !REJECTIONS
                .iter()
                .any(|(value, _)| *value == submitted.category)
            {
                return Err("Choose what is wrong with the claim.".to_owned());
            }
            Ok(with_parts("rejected", Some(submitted.category)))
        }
        "split" => {
            if !compound {
                return Err("Only compound claims have parts to review one by one.".to_owned());
            }
            Ok(vec![decision(claim_id, "rejected", Some(SPLIT.0), None)])
        }
        "flag" => {
            if compound {
                return Err("Only a compound claim's parts go back to the model.".to_owned());
            }
            if !FLAGS.iter().any(|(value, _)| *value == submitted.flag) {
                return Err("Choose why the claim goes back to the model.".to_owned());
            }
            Ok(vec![decision(
                claim_id,
                "flagged",
                Some(submitted.flag),
                None,
            )])
        }
        "edit" => {
            let text = submitted.text.trim();
            if text.is_empty() {
                return Err("A reworded claim needs text.".to_owned());
            }
            Ok(vec![decision(claim_id, "edited", None, Some(text))])
        }
        "details" => {
            let Some(latest) = latest else {
                return Err("Details go with a decision. Decide about the claim first.".to_owned());
            };
            let reason = submitted.reason.trim();
            if reason.is_empty() {
                return Err("There were no details to save.".to_owned());
            }
            Ok(vec![Decision {
                reason: Some(reason.to_owned()),
                reviewer: reviewer.to_owned(),
                ..latest.clone()
            }])
        }
        other => Err(format!("\"{other}\" is not a review action.")),
    }
}

/// One thing to review: a compound claim, or an atomic claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    pub claim_id: i64,
    pub compound: bool,
    /// The compound claim an atomic claim is part of.
    pub part_of: Option<i64>,
}

/// The order a file is reviewed in: each compound claim, then its parts,
/// and after all of those the claims outside compound claims. `compounds`
/// pairs each compound claim's ID with the IDs of its parts.
pub fn queue(atomic: &[i64], compounds: &[(i64, &[i64])]) -> Vec<Item> {
    let mut items = Vec::new();
    for (compound, parts) in compounds {
        items.push(Item {
            claim_id: *compound,
            compound: true,
            part_of: None,
        });
        // Parts no longer among the file's claims have nothing to review.
        for part in parts.iter().filter(|part| atomic.contains(part)) {
            items.push(Item {
                claim_id: *part,
                compound: false,
                part_of: Some(*compound),
            });
        }
    }
    for claim in atomic {
        if !items.iter().any(|item| item.claim_id == *claim) {
            items.push(Item {
                claim_id: *claim,
                compound: false,
                part_of: None,
            });
        }
    }
    items
}

/// How many items of a queue a reviewer has dealt with: decided or flagged.
pub fn handled(queue: &[Item], decisions: &[RecordedDecision]) -> usize {
    queue
        .iter()
        .filter(|item| standing(decisions, item.claim_id).state != State::Proposed)
        .count()
}

/// The next item without a decision: the first one after the item `after`,
/// going round to the start, or the first of the queue. `None` once every
/// item is dealt with.
pub fn next_pending(
    queue: &[Item],
    decisions: &[RecordedDecision],
    after: Option<i64>,
) -> Option<Item> {
    let start = after
        .and_then(|after| queue.iter().position(|item| item.claim_id == after))
        .map_or(0, |position| position + 1);
    queue
        .iter()
        .cycle()
        .skip(start)
        .take(queue.len())
        .find(|item| standing(decisions, item.claim_id).state == State::Proposed)
        .copied()
}
