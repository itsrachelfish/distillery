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

/// The words shown for a rejection or flag category.
pub fn label(category: &str) -> &str {
    REJECTIONS
        .iter()
        .chain(&FLAGS)
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

impl State {
    /// Whether a reviewer has settled the claim, one way or the other.
    pub fn decided(&self) -> bool {
        matches!(self, State::Confirmed | State::Rejected(_))
    }
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
    /// `confirm`, `edit`, `reject` or `flag`.
    pub action: &'a str,
    /// The new text, for edits.
    pub text: &'a str,
    /// Why, for rejections.
    pub category: &'a str,
    /// Why, for flags.
    pub flag: &'a str,
    /// The reviewer's own words, which every action may carry.
    pub reason: &'a str,
}

/// The decisions to record for a submission. Confirming or rejecting a
/// compound claim decides its `undecided_parts` the same way, so a group is
/// reviewed in one step; parts decided on their own keep their decisions.
/// Atomic claims have no parts.
pub fn decide(
    claim_id: i64,
    compound: bool,
    undecided_parts: &[i64],
    submitted: &Submitted,
    reviewer: &str,
) -> Result<Vec<Decision>, String> {
    let reason = Some(submitted.reason.trim())
        .filter(|reason| !reason.is_empty())
        .map(str::to_owned);
    let decision =
        |claim_id, decision: &str, category: Option<&str>, text: Option<&str>| Decision {
            claim_id,
            decision: decision.to_owned(),
            category: category.map(str::to_owned),
            text: text.map(str::to_owned),
            reason: reason.clone(),
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
                return Err("Choose why the claim is rejected.".to_owned());
            }
            Ok(with_parts("rejected", Some(submitted.category)))
        }
        "edit" => {
            let text = submitted.text.trim();
            if text.is_empty() {
                return Err("An edited claim needs text.".to_owned());
            }
            Ok(vec![decision(claim_id, "edited", None, Some(text))])
        }
        "flag" => {
            if compound {
                return Err("Flags are for a compound claim's parts.".to_owned());
            }
            if !FLAGS.iter().any(|(value, _)| *value == submitted.flag) {
                return Err("Choose why the claim is flagged.".to_owned());
            }
            Ok(vec![decision(
                claim_id,
                "flagged",
                Some(submitted.flag),
                None,
            )])
        }
        other => Err(format!("\"{other}\" is not a review action.")),
    }
}
