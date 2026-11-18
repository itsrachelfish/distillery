//! The extraction stage, pass 1: atomic claims from each section of a file.
//!
//! One call per section. Replies are held to the claims schema by the API,
//! and Distillery checks what the schema can't: every claim has to rest on
//! passages of its section. Replies with problems go back to the model once. Sections
//! that fail again, and replies cut off or refused, are left not distilled
//! with the reply kept. Distilled sections are cached, so later runs never
//! send them again.

use std::time::Duration;

use rusqlite::Connection;
use serde_json::Value;

use crate::config;
use crate::db::{self, CallSection, Claim, ModelCall, RunFile, SectionKey};
use crate::estimate::heading_only;
use crate::model::{self, ModelError, ModelProvider, Prompt, Reply};
use crate::passages::{self, Passage};
use crate::progress::{Progress, counted};
use crate::prompts::{self, Extraction};
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

/// The reply's keys. Single letters keep replies short, since every claim
/// repeats them.
mod key {
    pub const CLAIMS: &str = "c";
    pub const TEXT: &str = "t";
    pub const KIND: &str = "k";
    /// The number of the first passage a claim rests on.
    pub const FIRST: &str = "f";
    /// The number of the last passage.
    pub const LAST: &str = "l";
    pub const ILLUSTRATIVE: &str = "i";
}

/// A Markdown file at the run's commit.
pub struct File<'a> {
    pub path: &'a str,
    /// The hash of the file's contents, which the cache goes by.
    pub blob_hash: &'a str,
    pub contents: &'a str,
    /// The estimate the file is started from.
    pub estimate_id: Option<i64>,
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

/// Runs pass 1 over files of one source at one commit and records the run.
/// `settings` hold the prices and the cost limit.
pub fn run(
    conn: &mut Connection,
    provider: &dyn ModelProvider,
    settings: &config::Model,
    source_id: i64,
    commit: &str,
    files: &[File],
    progress: &mut Progress,
) -> Result<Ran, String> {
    let extraction = Extraction::load()?;
    let (system, schema, retry) = extraction.wording();
    db::record_prompt_version(conn, &extraction.version, system, schema, retry)
        .map_err(|err| format!("Recording the prompt version: {err}"))?;
    let run = db::Run {
        source_id,
        commit_hash: commit.to_owned(),
        provider: provider.provider().to_owned(),
        model: provider.model().to_owned(),
        prompt_version: extraction.version.clone(),
    };
    let run_files: Vec<RunFile> = files
        .iter()
        .map(|file| RunFile {
            path: file.path.to_owned(),
            blob_hash: file.blob_hash.to_owned(),
            estimate_id: file.estimate_id,
        })
        .collect();
    let run_id =
        db::start_run(conn, &run, &run_files).map_err(|err| format!("Recording the run: {err}"))?;

    let result = distill(
        conn,
        provider,
        settings,
        files,
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

/// What identifies a section to the cache.
fn section_key<'a>(
    file: &File<'a>,
    section: &Section,
    prompt_version: &'a str,
    model: &'a str,
) -> SectionKey<'a> {
    SectionKey {
        blob_hash: file.blob_hash,
        start_line: section.start_line as i64,
        end_line: section.end_line as i64,
        prompt_version,
        model,
    }
}

/// A section waiting to be sent, with its file and the file's lines.
struct Waiting<'a> {
    file: &'a File<'a>,
    section: Section,
    lines: &'a [&'a str],
}

fn distill(
    conn: &mut Connection,
    provider: &dyn ModelProvider,
    settings: &config::Model,
    files: &[File],
    extraction: &Extraction,
    run_id: i64,
    progress: &mut Progress,
) -> Result<Ran, String> {
    let mut ran = Ran {
        run_id,
        ..Ran::default()
    };
    let (version, model) = (extraction.version.as_str(), provider.model());
    let lines: Vec<Vec<&str>> = files
        .iter()
        .map(|file| file.contents.lines().collect())
        .collect();

    progress.stage("Checking the cache", files.len() as u64, "files");
    let mut to_send = Vec::new();
    for (index, file) in files.iter().enumerate() {
        progress.update(index as u64, || file.path.to_owned());
        for section in sections::split(file.contents) {
            if heading_only(&section) {
                continue;
            }
            let cached = db::cached_extraction(conn, &section_key(file, &section, version, model))
                .map_err(|err| format!("Reading the cache: {err}"))?;
            if cached.is_some() {
                ran.cached += 1;
            } else {
                to_send.push(Waiting {
                    file,
                    lines: &lines[index][section.start_line - 1..section.end_line],
                    section,
                });
            }
        }
    }
    progress.done(&format!(
        "{} cached, {} to send",
        counted(ran.cached as u64, "section", "sections"),
        to_send.len()
    ));

    progress.stage("Extracting claims", to_send.len() as u64, "sections");
    let limit = settings.max_run_cost_micros();
    for (index, waiting) in to_send.iter().enumerate() {
        if ran.cost_micros > limit {
            ran.unsent = to_send.len() - index;
            break;
        }
        let Waiting {
            file,
            section,
            lines,
        } = waiting;
        // Claims point at passages by number, so the model writes no quotes.
        let cut = passages::cut(lines, section.start_line);
        let sent = prompts::Section {
            lines,
            first_line: section.start_line,
            passages: &cut,
        };
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

        let prompt = extraction.prompt(file.path, &title, &sent);
        let first = call.send(conn, &prompt, &mut ran.cost_micros, progress)?;
        let mut judged = judge(first, &sent);
        if let Judged::Problems(problems, _) = &judged {
            // Rejected claims go back to the model once, with the reasons.
            let prompt = extraction.retry(file.path, &title, &sent, problems);
            let second = call.send(conn, &prompt, &mut ran.cost_micros, progress)?;
            judged = judge(second, &sent);
        }

        let (claims, reason, reply) = match judged {
            Judged::Claims(claims, reply) => (claims, None, reply),
            Judged::Problems(problems, reply) => (Vec::new(), Some(problems.join("\n")), reply),
            Judged::Incomplete(reason, reply) => (Vec::new(), Some(reason), reply),
        };
        db::record_extraction(
            conn,
            run_id,
            &section_key(file, section, version, model),
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
        let (cache_write, cache_read) = (
            usage.cache_write_tokens as i64,
            usage.cache_read_tokens as i64,
        );
        ModelCall {
            provider: self.provider.provider().to_owned(),
            model: self.provider.model().to_owned(),
            operation: prompts::EXTRACT_OPERATION.to_owned(),
            input_tokens: input,
            output_tokens: output,
            thinking_tokens: usage.thinking_tokens as i64,
            cache_write_tokens: cache_write,
            cache_read_tokens: cache_read,
            cost_micros: self
                .settings
                .call_cost_micros(input, cache_write, cache_read, output),
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

/// Checks a reply against its section.
fn judge(reply: Reply, section: &prompts::Section) -> Judged {
    if !reply.complete() {
        let reason = match reply.stop_reason.as_str() {
            "max_tokens" => "The reply was cut off at the output token limit.".to_owned(),
            "refusal" => "The model refused the section.".to_owned(),
            other => format!("The reply ended early, with stop reason {other}."),
        };
        return Judged::Incomplete(reason, reply.text);
    }
    match check(&reply.text, section) {
        Ok(claims) => Judged::Claims(claims, reply.text),
        Err(problems) => Judged::Problems(problems, reply.text),
    }
}

/// The claims of a complete reply, or every problem found in it. Each
/// claim's lines, columns and quote come from the passages it names.
fn check(reply: &str, section: &prompts::Section) -> Result<Vec<Claim>, Vec<String>> {
    let parsed: Value = serde_json::from_str(reply)
        .map_err(|err| vec![format!("The reply was not JSON: {err}.")])?;
    let Some(list) = parsed[key::CLAIMS].as_array() else {
        return Err(vec!["The reply had no list of claims.".to_owned()]);
    };
    let passages = section.passages;
    let mut claims = Vec::new();
    let mut problems = Vec::new();
    for (index, claim) in list.iter().enumerate() {
        let number = index + 1;
        let text = claim[key::TEXT].as_str().unwrap_or("").trim();
        // Capital letters in kinds aren't held to the schema's list.
        let kind = claim[key::KIND].as_str().unwrap_or("").to_lowercase();
        if text.is_empty() {
            problems.push(format!("Claim {number} has no text."));
        }
        // Pieces of the reply's own syntax inside a note mean the reply
        // went wrong there.
        if text.contains("\",\"") || text.contains("\":") {
            problems.push(format!("Claim {number} has JSON syntax in its text."));
        }
        if !KINDS.contains(&kind.as_str()) {
            problems.push(format!("Claim {number} has the unknown kind \"{kind}\"."));
        }
        let span = span(passages, &claim[key::FIRST], &claim[key::LAST]);
        if span.is_none() {
            problems.push(format!(
                "Claim {number}: passages {} to {} are not passages of the section, which has \
                 passages 1 to {}, first before last.",
                claim[key::FIRST],
                claim[key::LAST],
                passages.len()
            ));
        }
        if let (Some((from, to)), true) = (span, problems.is_empty()) {
            claims.push(Claim {
                text: text.to_owned(),
                kind,
                illustrative: claim[key::ILLUSTRATIVE].as_bool().unwrap_or(false),
                quote: passages::quote(section.lines, section.first_line, from, to),
                start_line: from.line as i64,
                end_line: to.line as i64,
                start_column: Some(from.start_column as i64),
                end_column: Some(to.end_column as i64),
            });
        }
    }
    if problems.is_empty() {
        Ok(claims)
    } else {
        Err(problems)
    }
}

/// The first and last passage a claim names, when both numbers are
/// passages of the section and in order.
fn span<'a>(
    passages: &'a [Passage],
    first: &Value,
    last: &Value,
) -> Option<(&'a Passage, &'a Passage)> {
    let (first, last) = (first.as_u64()? as usize, last.as_u64()? as usize);
    if first == 0 || first > last {
        return None;
    }
    Some((passages.get(first - 1)?, passages.get(last - 1)?))
}
