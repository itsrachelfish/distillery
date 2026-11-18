//! The extraction stage, pass 1: atomic claims from each section of a file.
//!
//! One call per section. Replies are held to the claims schema by the API,
//! and Distillery checks what the schema can't: every quote has to appear in
//! its section. Replies with problems go back to the model once. Sections
//! that fail again, and replies cut off or refused, are left not distilled
//! with the reply kept. Distilled sections are cached, so later runs never
//! send them again.

use std::time::Duration;

use rusqlite::Connection;
use serde_json::Value;

use crate::config;
use crate::db::{self, CallSection, Claim, ModelCall, SectionKey};
use crate::estimate::heading_only;
use crate::model::{self, ModelError, ModelProvider, Prompt, Reply};
use crate::progress::{Progress, counted};
use crate::prompts::{self, Extraction};
use crate::quotes;
use crate::sections::{self, Section};

/// The kinds of claims, as the reply schema lists them.
const KINDS: [&str; 6] = [
    "behavior",
    "configuration",
    "requirement",
    "decision",
    "instruction",
    "rationale",
];
/// How much of a rejected quote a problem repeats.
const QUOTED: usize = 120;

/// A Markdown file at a recorded commit.
pub struct File<'a> {
    pub source_id: i64,
    pub commit: &'a str,
    pub path: &'a str,
    /// The hash of the file's contents, which the cache goes by.
    pub blob_hash: &'a str,
    pub contents: &'a str,
}

/// What a run did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Ran {
    pub run_id: i64,
    /// Sections distilled by this run.
    pub distilled: usize,
    /// Sections left not distilled.
    pub failed: usize,
    /// Sections earlier runs distilled, which were not sent.
    pub cached: usize,
    /// Sections never sent, because the run stopped at its cost limit.
    pub unsent: usize,
    pub claims: usize,
    /// Millionths of a US dollar.
    pub cost_micros: i64,
}

/// Runs pass 1 over a file and records the run. `settings` hold the prices
/// and the cost limit. `estimate_id` is the estimate the run starts from.
pub fn run(
    conn: &mut Connection,
    provider: &dyn ModelProvider,
    settings: &config::Model,
    file: &File,
    estimate_id: Option<i64>,
    progress: &mut Progress,
) -> Result<Ran, String> {
    let extraction = Extraction::load()?;
    let run = db::Run {
        source_id: file.source_id,
        commit_hash: file.commit.to_owned(),
        provider: provider.provider().to_owned(),
        model: provider.model().to_owned(),
        prompt_version: extraction.version.clone(),
        estimate_id,
    };
    let run_id = db::start_run(conn, &run, file.path, file.blob_hash)
        .map_err(|err| format!("Recording the run: {err}"))?;

    let result = distill(
        conn,
        provider,
        settings,
        file,
        &extraction,
        run_id,
        progress,
    );
    let outcome = match &result {
        Ok(ran) if ran.unsent > 0 => format!(
            "Stopped at the cost limit with {} not sent",
            counted(ran.unsent as u64, "section", "sections")
        ),
        Ok(_) => "Finished".to_owned(),
        Err(err) => format!("Failed: {err}"),
    };
    db::finish_run(conn, run_id, &outcome).map_err(|err| format!("Recording the run: {err}"))?;
    result
}

fn distill(
    conn: &mut Connection,
    provider: &dyn ModelProvider,
    settings: &config::Model,
    file: &File,
    extraction: &Extraction,
    run_id: i64,
    progress: &mut Progress,
) -> Result<Ran, String> {
    let mut ran = Ran {
        run_id,
        ..Ran::default()
    };
    let lines: Vec<&str> = file.contents.lines().collect();
    let all = sections::split(file.contents);
    let key = |section: &Section| SectionKey {
        blob_hash: file.blob_hash,
        start_line: section.start_line as i64,
        end_line: section.end_line as i64,
        prompt_version: &extraction.version,
        model: provider.model(),
    };

    progress.stage("Checking the cache", 0, "sections");
    let mut to_send = Vec::new();
    for section in all.iter().filter(|section| !heading_only(section)) {
        let cached = db::cached_extraction(conn, &key(section))
            .map_err(|err| format!("Reading the cache: {err}"))?;
        if cached.is_some() {
            ran.cached += 1;
        } else {
            to_send.push(section);
        }
    }
    progress.done(&format!(
        "{} cached, {} to send",
        counted(ran.cached as u64, "section", "sections"),
        to_send.len()
    ));

    progress.stage("Extracting claims", to_send.len() as u64, "sections");
    let limit = settings.max_run_cost_micros();
    for (index, section) in to_send.iter().enumerate() {
        if ran.cost_micros > limit {
            ran.unsent = to_send.len() - index;
            break;
        }
        let section_lines = &lines[section.start_line - 1..section.end_line];
        // Sections are sent as written.
        let text = section_lines.join("\n");
        let title = section.title();
        let call = Call {
            provider,
            settings,
            section: CallSection {
                run_id,
                path: file.path.to_owned(),
                start_line: section.start_line as i64,
                end_line: section.end_line as i64,
            },
            done: index as u64,
        };
        progress.update(call.done, || call.place());

        let prompt = extraction.prompt(file.path, &title, &text);
        let first = call.send(conn, &prompt, &mut ran.cost_micros, progress)?;
        let mut judged = judge(first, section_lines, section.start_line);
        if let Judged::Problems(problems, _) = &judged {
            // Rejected claims go back to the model once, with the reasons.
            let prompt = extraction.retry(file.path, &title, &text, problems);
            let second = call.send(conn, &prompt, &mut ran.cost_micros, progress)?;
            judged = judge(second, section_lines, section.start_line);
        }

        let (claims, reason, reply) = match judged {
            Judged::Claims(claims, reply) => (claims, None, reply),
            Judged::Problems(problems, reply) => (Vec::new(), Some(problems.join("\n")), reply),
            Judged::Incomplete(reason, reply) => (Vec::new(), Some(reason), reply),
        };
        db::record_extraction(
            conn,
            run_id,
            &key(section),
            &claims,
            reason.as_deref(),
            &reply,
        )
        .map_err(|err| format!("Recording the claims: {err}"))?;
        if reason.is_none() {
            ran.distilled += 1;
            ran.claims += claims.len();
        } else {
            ran.failed += 1;
        }
    }
    progress.done(&format!(
        "{} from {}, {} not distilled",
        counted(ran.claims as u64, "claim", "claims"),
        counted(ran.distilled as u64, "section", "sections"),
        ran.failed
    ));
    Ok(ran)
}

/// One section's calls to the model.
struct Call<'a> {
    provider: &'a dyn ModelProvider,
    settings: &'a config::Model,
    section: CallSection,
    /// Sections done so far, for the progress line.
    done: u64,
}

impl Call<'_> {
    /// The file and lines, for progress and errors.
    fn place(&self) -> String {
        format!(
            "{} lines {}–{}",
            self.section.path, self.section.start_line, self.section.end_line
        )
    }

    /// Sends a prompt, attempting again after rate limits and server
    /// errors. Every attempt is recorded as a model call, and its cost is
    /// added to `cost_micros`.
    fn send(
        &self,
        conn: &Connection,
        prompt: &Prompt,
        cost_micros: &mut i64,
        progress: &mut Progress,
    ) -> Result<Reply, String> {
        model::attempt(
            progress,
            self.done,
            &format!("Extracting claims from {}", self.place()),
            || self.provider.reply(prompt),
            |took, reply| {
                let call = self.record(took, reply);
                *cost_micros += call.cost_micros;
                db::record_model_call(conn, &call)
                    .map_err(|err| format!("Recording the call: {err}"))
            },
        )
    }

    /// One attempt as a usage record. Cost comes from the configured prices
    /// at the time of the call.
    fn record(&self, took: Duration, reply: Result<&Reply, &ModelError>) -> ModelCall {
        let usage = reply.map(|reply| reply.usage).unwrap_or_default();
        let (input, output) = (usage.input_tokens as i64, usage.output_tokens as i64);
        ModelCall {
            provider: self.provider.provider().to_owned(),
            model: self.provider.model().to_owned(),
            operation: prompts::EXTRACT_OPERATION.to_owned(),
            input_tokens: input,
            output_tokens: output,
            thinking_tokens: usage.thinking_tokens as i64,
            cache_write_tokens: usage.cache_write_tokens as i64,
            cache_read_tokens: usage.cache_read_tokens as i64,
            cost_micros: self.settings.input_cost_micros(input)
                + self.settings.output_cost_micros(output),
            duration_ms: took.as_millis() as i64,
            stop_reason: reply.ok().map(|reply| reply.stop_reason.clone()),
            error: reply.err().map(|error| error.message.clone()),
            http_status: reply.err().and_then(|error| error.status).map(i64::from),
            section: Some(self.section.clone()),
        }
    }
}

/// A reply after checking, each with the reply's text.
enum Judged {
    /// Every claim passed.
    Claims(Vec<Claim>, String),
    /// What is wrong with the claims, in words the model can act on.
    Problems(Vec<String>, String),
    /// The reply was cut off or refused, so the schema doesn't hold for it.
    Incomplete(String, String),
}

/// Checks a reply against its section. `first_line` is the line number of
/// the first of `lines`.
fn judge(reply: Reply, lines: &[&str], first_line: usize) -> Judged {
    if !reply.complete() {
        let reason = match reply.stop_reason.as_str() {
            "max_tokens" => "The reply was cut off at the output token limit.".to_owned(),
            "refusal" => "The model refused the section.".to_owned(),
            other => format!("The reply ended early, with stop reason {other}."),
        };
        return Judged::Incomplete(reason, reply.text);
    }
    match check(&reply.text, lines, first_line) {
        Ok(claims) => Judged::Claims(claims, reply.text),
        Err(problems) => Judged::Problems(problems, reply.text),
    }
}

/// The claims of a complete reply, or every problem found in it.
fn check(reply: &str, lines: &[&str], first_line: usize) -> Result<Vec<Claim>, Vec<String>> {
    let parsed: Value = serde_json::from_str(reply)
        .map_err(|err| vec![format!("The reply was not JSON: {err}.")])?;
    let Some(list) = parsed["claims"].as_array() else {
        return Err(vec!["The reply had no list of claims.".to_owned()]);
    };
    let mut claims = Vec::new();
    let mut problems = Vec::new();
    for (index, claim) in list.iter().enumerate() {
        let number = index + 1;
        let text = claim["text"].as_str().unwrap_or("").trim();
        let quote = claim["quote"].as_str().unwrap_or("");
        // Capital letters in kinds aren't held to the schema's list.
        let kind = claim["kind"].as_str().unwrap_or("").to_lowercase();
        if text.is_empty() {
            problems.push(format!("Claim {number} has no text."));
        }
        if !KINDS.contains(&kind.as_str()) {
            problems.push(format!("Claim {number} has the unknown kind \"{kind}\"."));
        }
        let found = quotes::find(lines, first_line, quote);
        if found.is_none() {
            let shown: String = quote.chars().take(QUOTED).collect();
            problems.push(format!(
                "Claim {number}: the quote \"{shown}\" does not appear in the section."
            ));
        }
        if let (Some((start_line, end_line)), true) = (found, problems.is_empty()) {
            claims.push(Claim {
                text: text.to_owned(),
                kind,
                illustrative: claim["illustrative"].as_bool().unwrap_or(false),
                quote: quote.to_owned(),
                start_line: start_line as i64,
                end_line: end_line as i64,
            });
        }
    }
    if problems.is_empty() {
        Ok(claims)
    } else {
        Err(problems)
    }
}
