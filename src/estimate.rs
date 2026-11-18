//! The estimate stage: what distilling a file would send to the model,
//! counted before any run.
//!
//! Each section is counted as its own call, with the prompt and the reply
//! schema, since that is what a run sends. Providers count for free. The
//! estimate is recorded and never changed, so the cost of a run can be
//! compared with what was expected.

use std::thread;
use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::config;
use crate::db::{self, Estimate, EstimateSection, ModelCall};
use crate::model::{self, ModelError, ModelProvider, Prompt};
use crate::progress::{Progress, counted, thousands};
use crate::prompts::{self, Extraction};
use crate::sections::{self, Section};

/// What usage records call token counts.
const OPERATION: &str = "count_tokens";

/// A Markdown file at a recorded commit.
pub struct File<'a> {
    pub source_id: i64,
    pub commit: &'a str,
    pub path: &'a str,
    pub contents: &'a str,
}

/// A recorded estimate.
pub struct Estimated {
    pub id: i64,
    /// How many sections were counted, one call each.
    pub sections: usize,
    pub estimate: Estimate,
}

/// Counts the tokens of every section of a file and records the estimate.
/// `prices` are the model settings the costs come from.
pub fn run(
    conn: &mut Connection,
    provider: &dyn ModelProvider,
    prices: &config::Model,
    file: &File,
    progress: &mut Progress,
) -> Result<Estimated, String> {
    progress.stage("Cutting sections", 0, "sections");
    let lines: Vec<&str> = file.contents.lines().collect();
    let all = sections::split(file.contents);
    let (skipped, to_send): (Vec<&Section>, Vec<&Section>) =
        all.iter().partition(|section| heading_only(section));
    progress.done(&format!(
        "{}, {} to send",
        counted(all.len() as u64, "section", "sections"),
        thousands(to_send.len() as u64)
    ));
    if to_send.is_empty() {
        return Err(format!("{} has no text to distill.", file.path));
    }

    let extraction = Extraction::load()?;
    progress.stage("Counting tokens", to_send.len() as u64, "sections");
    let mut sections = Vec::with_capacity(to_send.len());
    for (index, section) in to_send.iter().enumerate() {
        // Sections are sent as written.
        let text = lines[section.start_line - 1..section.end_line].join("\n");
        let prompt = extraction.prompt(file.path, &section.title(), &text);
        let place = format!(
            "{} lines {}–{}",
            file.path, section.start_line, section.end_line
        );
        progress.update(index as u64, || place.clone());
        let input_tokens = count(conn, provider, &prompt, index as u64, &place, progress)?;
        sections.push(EstimateSection {
            start_line: section.start_line as i64,
            end_line: section.end_line as i64,
            input_tokens,
        });
    }
    let input_tokens: i64 = sections.iter().map(|section| section.input_tokens).sum();
    progress.done(&format!(
        "{} in {}",
        counted(input_tokens as u64, "input token", "input tokens"),
        counted(sections.len() as u64, "call", "calls")
    ));

    // Output can't be counted in advance. It is expected from what the
    // model's earlier extraction calls returned for their input.
    let ratio = db::output_ratio(conn, prompts::EXTRACT_OPERATION, provider.model())
        .map_err(|err| format!("Reading earlier calls: {err}"))?;
    let output_tokens = ratio.output_for(input_tokens);
    let estimate = Estimate {
        source_id: file.source_id,
        commit_hash: file.commit.to_owned(),
        path: file.path.to_owned(),
        provider: provider.provider().to_owned(),
        model: provider.model().to_owned(),
        prompt_version: extraction.version,
        skipped_sections: skipped.len() as i64,
        input_tokens,
        input_cost_micros: prices.input_cost_micros(input_tokens),
        output_tokens,
        output_cost_micros: output_tokens.map(|tokens| prices.output_cost_micros(tokens)),
        ratio_calls: ratio.calls,
    };
    let id = db::record_estimate(conn, &estimate, &sections)
        .map_err(|err| format!("Recording the estimate: {err}"))?;
    Ok(Estimated {
        id,
        sections: sections.len(),
        estimate,
    })
}

/// Whether a section holds a one-line heading and nothing under it, so no
/// call is needed for it.
fn heading_only(section: &Section) -> bool {
    !section.path.is_empty() && section.start_line == section.end_line
}

/// Counts one prompt's input tokens, attempting again after rate limits and
/// server errors. Every attempt is recorded as a model call.
fn count(
    conn: &Connection,
    provider: &dyn ModelProvider,
    prompt: &Prompt,
    done: u64,
    place: &str,
    progress: &mut Progress,
) -> Result<i64, String> {
    let mut attempt = 1;
    loop {
        let started = Instant::now();
        let reply = provider.count_tokens(prompt);
        record(conn, provider, started.elapsed(), reply.as_ref().err())?;
        let error = match reply {
            Ok(tokens) => return Ok(tokens as i64),
            Err(error) => error,
        };
        let Some(wait) = model::retry_wait(attempt, &error) else {
            return Err(format!(
                "Counting {place} failed after {}: {error}",
                counted(attempt.into(), "attempt", "attempts")
            ));
        };
        attempt += 1;
        let until = Instant::now() + wait;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            progress.update(done, || {
                format!(
                    "{place}, attempt {attempt} of {} in {}s: {error}",
                    model::MAX_ATTEMPTS,
                    left.as_secs() + 1
                )
            });
            thread::sleep(left.min(Duration::from_secs(1)));
        }
    }
}

/// Records one attempt at counting tokens.
fn record(
    conn: &Connection,
    provider: &dyn ModelProvider,
    took: Duration,
    error: Option<&ModelError>,
) -> Result<(), String> {
    let call = ModelCall {
        provider: provider.provider().to_owned(),
        model: provider.model().to_owned(),
        operation: OPERATION.to_owned(),
        // Counting is free and runs no model, so no tokens are used.
        input_tokens: 0,
        output_tokens: 0,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        cost_micros: 0,
        duration_ms: took.as_millis() as i64,
        stop_reason: None,
        error: error.map(|error| error.message.clone()),
        http_status: error.and_then(|error| error.status).map(i64::from),
    };
    db::record_model_call(conn, &call).map_err(|err| format!("Recording the call: {err}"))
}
