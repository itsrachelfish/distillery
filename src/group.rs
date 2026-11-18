//! The grouping stage, pass 2: a file's claims grouped into compound claims,
//! with the relations between claims.
//!
//! Calls cost money, so the stage makes as few as it can: one per file,
//! never for files with too few claims to group, and never twice for the
//! same claims. Claims go in numbered from 1, and the reply names them by
//! number, so the model repeats no claim text.

use std::collections::HashSet;
use std::time::Duration;

use rusqlite::Connection;
use serde_json::Value;

use crate::config;
use crate::db::{self, Compound, Link, ModelCall, ShownClaim};
use crate::git;
use crate::model::{self, ModelError, ModelProvider, Prompt, Reply};
use crate::progress::{Progress, counted};
use crate::prompts::{self, Grouping, Listed};

/// The fewest claims worth a call. Compound claims need two parts, and two
/// claims alone leave nothing to choose.
pub const FEWEST_CLAIMS: usize = 3;

/// The kinds of claims, as the reply schema lists them.
const KINDS: [&str; 6] = [
    "behavior",
    "configuration",
    "requirement",
    "decision",
    "instruction",
    "rationale",
];
/// The relations asked for, as the reply schema lists them.
const RELATIONS: [&str; 3] = ["duplicates", "conflicts", "depends_on"];

/// The reply's keys. Single letters keep replies short.
mod key {
    pub const GROUPS: &str = "g";
    pub const TEXT: &str = "t";
    /// The kind of a compound claim, or the relation of a link.
    pub const KIND: &str = "k";
    pub const PARTS: &str = "p";
    pub const RELATIONS: &str = "r";
    pub const FIRST: &str = "a";
    pub const SECOND: &str = "b";
}

/// A Markdown file whose claims are grouped.
pub struct File<'a> {
    pub path: &'a str,
    /// The hash of the file's contents, which claims and groupings go by.
    pub blob_hash: &'a str,
}

/// What a grouping job did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Grouped {
    /// Files grouped by this job.
    pub grouped: usize,
    /// Files whose reply stayed invalid.
    pub failed: usize,
    /// Files grouped before, with the same claims.
    pub cached: usize,
    /// Files with too few claims to group.
    pub skipped: usize,
    /// Files never sent, because the job stopped at the cost limit.
    pub unsent: usize,
    pub compounds: usize,
    pub links: usize,
    /// Millionths of a US dollar.
    pub cost_micros: i64,
}

/// The hash that stands for a set of claims: `git hash-object` of their IDs.
pub fn claims_hash(claims: &[ShownClaim]) -> Result<String, String> {
    let ids: Vec<String> = claims.iter().map(|shown| shown.id.to_string()).collect();
    git::hash_object(ids.join(",").as_bytes())
}

/// Groups the claims of each file that has enough of them and isn't grouped
/// yet. `settings` hold the prices and the cost limit.
pub fn run(
    conn: &mut Connection,
    provider: &dyn ModelProvider,
    settings: &config::Model,
    files: &[File],
    progress: &mut Progress,
) -> Result<Grouped, String> {
    let grouping = Grouping::load()?;
    let (system, schema, retry) = grouping.wording();
    db::record_prompt_version(conn, &grouping.version, system, schema, retry)
        .map_err(|err| format!("Recording the prompt version: {err}"))?;
    let mut done = Grouped::default();

    progress.stage("Checking the cache", files.len() as u64, "files");
    let mut to_send = Vec::new();
    for (index, file) in files.iter().enumerate() {
        progress.update(index as u64, || file.path.to_owned());
        let claims = db::claims_of_blob(conn, file.blob_hash)
            .map_err(|err| format!("Reading the claims: {err}"))?;
        if claims.len() < FEWEST_CLAIMS {
            done.skipped += 1;
            continue;
        }
        let hash = claims_hash(&claims)?;
        let cached = db::is_grouped(
            conn,
            file.blob_hash,
            &hash,
            &grouping.version,
            provider.model(),
        )
        .map_err(|err| format!("Reading the cache: {err}"))?;
        if cached {
            done.cached += 1;
        } else {
            to_send.push((file, claims, hash));
        }
    }
    progress.done(&format!(
        "{} to send, {} cached, {} with too few claims",
        counted(to_send.len() as u64, "file", "files"),
        done.cached,
        done.skipped
    ));

    progress.stage("Grouping claims", to_send.len() as u64, "files");
    let limit = settings.max_run_cost_micros();
    for (index, (file, claims, hash)) in to_send.iter().enumerate() {
        if done.cost_micros > limit {
            done.unsent = to_send.len() - index;
            break;
        }
        progress.update(index as u64, || {
            format!(
                "{}, {}",
                file.path,
                counted(claims.len() as u64, "claim", "claims")
            )
        });
        let listed: Vec<Listed> = claims
            .iter()
            .map(|shown| Listed {
                kind: &shown.claim.kind,
                text: &shown.claim.text,
                quote: &shown.claim.quote,
            })
            .collect();
        let mut used = Used::default();
        let call = Call {
            provider,
            settings,
            place: format!("Grouping the claims of {}", file.path),
            done: index as u64,
        };

        let prompt = grouping.prompt(file.path, &listed);
        let first = call.send(conn, &prompt, &mut used, progress)?;
        let mut judged = judge(first, claims);
        if let Judged::Problems(problems, _) = &judged {
            // Invalid replies go back to the model once, with the reasons.
            let prompt = grouping.retry(file.path, &listed, problems);
            let second = call.send(conn, &prompt, &mut used, progress)?;
            judged = judge(second, claims);
        }

        let (compounds, links, reason, reply) = match judged {
            Judged::Groups(compounds, links, reply) => (compounds, links, None, reply),
            Judged::Problems(problems, reply) => {
                (Vec::new(), Vec::new(), Some(problems.join("\n")), reply)
            }
            Judged::Incomplete(reason, reply) => (Vec::new(), Vec::new(), Some(reason), reply),
        };
        done.cost_micros += used.cost_micros;
        if reason.is_none() {
            done.grouped += 1;
            done.compounds += compounds.len();
            done.links += links.len();
        } else {
            done.failed += 1;
        }
        let record = db::Grouping {
            blob_hash: file.blob_hash.to_owned(),
            claims_hash: hash.clone(),
            prompt_version: grouping.version.clone(),
            model: provider.model().to_owned(),
            reason,
            reply,
            calls: used.calls,
            input_tokens: used.input_tokens,
            output_tokens: used.output_tokens,
            cost_micros: used.cost_micros,
        };
        db::record_grouping(conn, &record, &compounds, &links)
            .map_err(|err| format!("Recording the grouping: {err}"))?;
    }
    progress.done(&format!(
        "{} in {}, {} not grouped",
        counted(done.compounds as u64, "compound claim", "compound claims"),
        counted(done.grouped as u64, "file", "files"),
        done.failed
    ));
    Ok(done)
}

/// What one file's calls used together.
#[derive(Default)]
struct Used {
    calls: i64,
    /// Input tokens, cached ones included.
    input_tokens: i64,
    output_tokens: i64,
    cost_micros: i64,
}

/// One file's calls to the model.
struct Call<'a> {
    provider: &'a dyn ModelProvider,
    settings: &'a config::Model,
    /// What the call is for, for progress and errors.
    place: String,
    /// Files done so far, for the progress line.
    done: u64,
}

impl Call<'_> {
    /// Sends a prompt, attempting again after rate limits and server
    /// errors. Every attempt is recorded as a model call and added to
    /// `used`.
    fn send(
        &self,
        conn: &Connection,
        prompt: &Prompt,
        used: &mut Used,
        progress: &mut Progress,
    ) -> Result<Reply, String> {
        model::attempt(
            progress,
            self.done,
            &self.place,
            || self.provider.reply(prompt),
            |took, reply| {
                let call = self.record(took, reply);
                used.calls += 1;
                used.input_tokens +=
                    call.input_tokens + call.cache_write_tokens + call.cache_read_tokens;
                used.output_tokens += call.output_tokens;
                used.cost_micros += call.cost_micros;
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
            operation: prompts::GROUP_OPERATION.to_owned(),
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
            section: None,
        }
    }
}

/// A reply after checking, each with the reply's text.
enum Judged {
    /// Every compound claim and link passed.
    Groups(Vec<Compound>, Vec<Link>, String),
    /// What is wrong with the reply, in words the model can act on.
    Problems(Vec<String>, String),
    /// The reply was cut off or refused, so the schema doesn't hold for it.
    Incomplete(String, String),
}

/// Checks a reply against the claims that were sent.
fn judge(reply: Reply, claims: &[ShownClaim]) -> Judged {
    if !reply.complete() {
        let reason = match reply.stop_reason.as_str() {
            "max_tokens" => "The reply was cut off at the output token limit.".to_owned(),
            "refusal" => "The model refused the claims.".to_owned(),
            other => format!("The reply ended early, with stop reason {other}."),
        };
        return Judged::Incomplete(reason, reply.text);
    }
    match check(&reply.text, claims) {
        Ok((compounds, links)) => Judged::Groups(compounds, links, reply.text),
        Err(problems) => Judged::Problems(problems, reply.text),
    }
}

/// The compound claims and links of a complete reply, with claim numbers
/// turned into claim IDs, or every problem found in it.
fn check(reply: &str, claims: &[ShownClaim]) -> Result<(Vec<Compound>, Vec<Link>), Vec<String>> {
    let parsed: Value = serde_json::from_str(reply)
        .map_err(|err| vec![format!("The reply was not JSON: {err}.")])?;
    let (Some(groups), Some(relations)) = (
        parsed[key::GROUPS].as_array(),
        parsed[key::RELATIONS].as_array(),
    ) else {
        return Err(vec![
            "The reply had no lists of compound claims and relations.".to_owned(),
        ]);
    };
    // The ID of the claim with a number, when the number was in the list.
    let id = |number: &Value| {
        let number = number.as_u64()? as usize;
        Some(claims.get(number.checked_sub(1)?)?.id)
    };
    let mut problems = Vec::new();

    let mut compounds = Vec::new();
    let mut taken = HashSet::new();
    for (index, group) in groups.iter().enumerate() {
        let number = index + 1;
        let text = group[key::TEXT].as_str().unwrap_or("").trim();
        // Capital letters in kinds aren't held to the schema's list.
        let kind = group[key::KIND].as_str().unwrap_or("").to_lowercase();
        if text.is_empty() {
            problems.push(format!("Compound claim {number} has no text."));
        }
        if text.contains("\",\"") || text.contains("\":") {
            problems.push(format!(
                "Compound claim {number} has JSON syntax in its text."
            ));
        }
        if !KINDS.contains(&kind.as_str()) {
            problems.push(format!(
                "Compound claim {number} has the unknown kind \"{kind}\"."
            ));
        }
        let mut parts = Vec::new();
        for part in group[key::PARTS].as_array().into_iter().flatten() {
            match id(part) {
                None => problems.push(format!(
                    "Compound claim {number}: {part} is not the number of a claim in the list, \
                     which has claims 1 to {}.",
                    claims.len()
                )),
                Some(id) if !taken.insert(id) => problems.push(format!(
                    "Compound claim {number}: claim {part} is a part more than once. Each \
                     claim is part of at most one compound claim."
                )),
                Some(id) => parts.push(id),
            }
        }
        if parts.len() < 2 {
            problems.push(format!(
                "Compound claim {number} has fewer than two valid parts."
            ));
        }
        compounds.push(Compound {
            text: text.to_owned(),
            kind,
            parts,
        });
    }

    let mut links = Vec::new();
    for (index, relation) in relations.iter().enumerate() {
        let number = index + 1;
        let kind = relation[key::KIND].as_str().unwrap_or("").to_lowercase();
        if !RELATIONS.contains(&kind.as_str()) {
            problems.push(format!(
                "Relation {number} has the unknown kind \"{kind}\"."
            ));
        }
        match (id(&relation[key::FIRST]), id(&relation[key::SECOND])) {
            (Some(from_id), Some(to_id)) if from_id != to_id => links.push(Link {
                from_id,
                to_id,
                relation: kind,
            }),
            _ => problems.push(format!(
                "Relation {number}: {} and {} are not two different claims of the list, which \
                 has claims 1 to {}.",
                relation[key::FIRST],
                relation[key::SECOND],
                claims.len()
            )),
        }
    }

    if problems.is_empty() {
        Ok((compounds, links))
    } else {
        Err(problems)
    }
}
