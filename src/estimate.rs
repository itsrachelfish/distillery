//! The estimate stage: what distilling a file would send to the model,
//! counted before any run.
//!
//! Each section is counted as its own call, with the prompt and the reply
//! schema, since that is what a run sends. Providers count for free. The
//! estimate is recorded and never changed, so the cost of a run can be
//! compared with what was expected.

use std::time::Duration;

use rusqlite::Connection;

use crate::config;
use crate::db::{self, Estimate, EstimateSection, ModelCall};
use crate::model::{self, ModelError, ModelProvider, Prompt};
use crate::passages;
use crate::progress::{Progress, counted};
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

/// What counting several files gave.
pub struct Counted {
    /// One estimate per file with text to distill, in the order given.
    pub estimates: Vec<Estimated>,
    /// The paths of files without text to distill, which get no estimate.
    pub empty: Vec<String>,
}

/// Counts the tokens of every section of the files and records one estimate
/// per file. `prices` are the model settings the costs come from.
pub fn run(
    conn: &mut Connection,
    provider: &dyn ModelProvider,
    prices: &config::Model,
    files: &[File],
    progress: &mut Progress,
) -> Result<Counted, String> {
    progress.stage("Cutting sections", files.len() as u64, "files");
    let mut cut = Vec::new();
    let mut empty = Vec::new();
    let mut total = 0;
    for (index, file) in files.iter().enumerate() {
        progress.update(index as u64, || file.path.to_owned());
        let all = sections::split(file.contents);
        let (skipped, to_send): (Vec<Section>, Vec<Section>) =
            all.into_iter().partition(heading_only);
        if to_send.is_empty() {
            empty.push(file.path.to_owned());
        } else {
            total += to_send.len();
            cut.push((file, skipped.len(), to_send));
        }
    }
    progress.done(&format!(
        "{} to send in {}",
        counted(total as u64, "section", "sections"),
        counted(cut.len() as u64, "file", "files")
    ));
    if cut.is_empty() {
        return Err("The files have no text to distill.".to_owned());
    }

    let extraction = Extraction::load()?;
    // Output can't be counted in advance. It is expected from what the
    // model's earlier extraction calls returned for their input.
    // Calls of this prompt version are the better guide, once it has any.
    let ratio = |version| {
        db::output_ratio(conn, prompts::EXTRACT_OPERATION, provider.model(), version)
            .map_err(|err| format!("Reading earlier calls: {err}"))
    };
    let ratio = match ratio(Some(extraction.version.as_str()))? {
        this_version if this_version.calls > 0 => this_version,
        _ => ratio(None)?,
    };

    progress.stage("Counting tokens", total as u64, "sections");
    let mut estimates = Vec::with_capacity(cut.len());
    let mut done = 0;
    let mut all_tokens = 0;
    for (file, skipped, to_send) in cut {
        let lines: Vec<&str> = file.contents.lines().collect();
        let mut sections = Vec::with_capacity(to_send.len());
        for section in &to_send {
            let section_lines = &lines[section.start_line - 1..section.end_line];
            let cut = passages::cut(section_lines, section.start_line);
            let sent = prompts::Section {
                lines: section_lines,
                first_line: section.start_line,
                passages: &cut,
            };
            let prompt = extraction.prompt(file.path, &section.title(), &sent);
            let place = format!(
                "{} lines {}–{}",
                file.path, section.start_line, section.end_line
            );
            progress.update(done, || place.clone());
            let input_tokens = count(conn, provider, &prompt, done, &place, progress)?;
            done += 1;
            sections.push(EstimateSection {
                start_line: section.start_line as i64,
                end_line: section.end_line as i64,
                input_tokens,
            });
        }
        let input_tokens: i64 = sections.iter().map(|section| section.input_tokens).sum();
        all_tokens += input_tokens;
        let output_tokens = ratio.output_for(input_tokens);
        let estimate = Estimate {
            source_id: file.source_id,
            commit_hash: file.commit.to_owned(),
            path: file.path.to_owned(),
            provider: provider.provider().to_owned(),
            model: provider.model().to_owned(),
            prompt_version: extraction.version.clone(),
            skipped_sections: skipped as i64,
            input_tokens,
            input_cost_micros: prices.input_cost_micros(input_tokens),
            output_tokens,
            output_cost_micros: output_tokens.map(|tokens| prices.output_cost_micros(tokens)),
            ratio_calls: ratio.calls,
        };
        // Each file's estimate is recorded as soon as it is counted, so a
        // failure further on keeps the files before it.
        let id = db::record_estimate(conn, &estimate, &sections)
            .map_err(|err| format!("Recording the estimate: {err}"))?;
        estimates.push(Estimated {
            id,
            sections: sections.len(),
            estimate,
        });
    }
    progress.done(&format!(
        "{} in {}",
        counted(all_tokens as u64, "input token", "input tokens"),
        counted(done, "call", "calls")
    ));
    Ok(Counted { estimates, empty })
}

/// Whether a section holds a one-line heading and nothing under it, so no
/// call is needed for it.
pub fn heading_only(section: &Section) -> bool {
    !section.path.is_empty() && section.start_line == section.end_line
}

/// Counts one prompt's input tokens. Every attempt is recorded as a model
/// call.
fn count(
    conn: &Connection,
    provider: &dyn ModelProvider,
    prompt: &Prompt,
    done: u64,
    place: &str,
    progress: &mut Progress,
) -> Result<i64, String> {
    let tokens = model::attempt(
        progress,
        done,
        &format!("Counting {place}"),
        || provider.count_tokens(prompt),
        |took, reply| record(conn, provider, took, reply.err()),
    )?;
    Ok(tokens as i64)
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
        thinking_tokens: 0,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        cost_micros: 0,
        duration_ms: took.as_millis() as i64,
        stop_reason: None,
        error: error.map(|error| error.message.clone()),
        http_status: error.and_then(|error| error.status).map(i64::from),
        section: None,
    };
    db::record_model_call(conn, &call).map_err(|err| format!("Recording the call: {err}"))
}
