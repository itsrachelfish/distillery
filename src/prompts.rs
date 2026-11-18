//! What models are asked: the prompts and reply schemas in `prompts/`,
//! tracked in git and built into the binary.
//!
//! The prompt version is the hash git gives the files, so records say which
//! wording produced them, and records from older wording show as needing
//! another run.

use serde_json::Value;

use crate::git;
use crate::model::Prompt;
use crate::passages::{self, Passage};

const EXTRACT_CLAIMS: &str = include_str!("../prompts/extract_claims.md");
const CLAIMS_SCHEMA: &str = include_str!("../prompts/claims_schema.json");
const RETRY: &str = include_str!("../prompts/extract_claims_retry.md");
/// Where the retry prompt lists the problems.
const PROBLEMS: &str = "{problems}";

/// What usage records call the calls that extract claims.
pub const EXTRACT_OPERATION: &str = "extract_claims";

const GROUP_CLAIMS: &str = include_str!("../prompts/group_claims.md");
const GROUPS_SCHEMA: &str = include_str!("../prompts/groups_schema.json");
const GROUP_RETRY: &str = include_str!("../prompts/group_claims_retry.md");

/// What usage records call the calls that group claims.
pub const GROUP_OPERATION: &str = "group_claims";

/// A message with a retry prompt under it, listing the problems of an
/// earlier reply.
fn with_problems(message: &mut String, retry: &str, problems: &[String]) {
    let list: Vec<String> = problems
        .iter()
        .map(|problem| format!("- {problem}"))
        .collect();
    message.push_str("\n\n");
    message.push_str(&retry.trim_end().replace(PROBLEMS, &list.join("\n")));
}

/// A claim as grouping calls list it.
pub struct Listed<'a> {
    pub kind: &'a str,
    pub text: &'a str,
    /// The source text the claim rests on.
    pub quote: &'a str,
}

/// Pass 2: the claims of one file grouped into compound claims, one call
/// per file.
pub struct Grouping {
    system: String,
    schema: Value,
    /// The same hash as `cat prompts/group_claims.md
    /// prompts/groups_schema.json prompts/group_claims_retry.md | git
    /// hash-object --stdin`.
    pub version: String,
}

impl Grouping {
    pub fn load() -> Result<Self, String> {
        let schema: Value = serde_json::from_str(GROUPS_SCHEMA)
            .map_err(|err| format!("prompts/groups_schema.json: {err}"))?;
        let version =
            git::hash_object(format!("{GROUP_CLAIMS}{GROUPS_SCHEMA}{GROUP_RETRY}").as_bytes())?;
        Ok(Grouping {
            system: GROUP_CLAIMS.trim_end().to_owned(),
            schema,
            version,
        })
    }

    /// The wording of this version: the system prompt, the reply schema and
    /// the retry prompt.
    pub fn wording(&self) -> (&'static str, &'static str, &'static str) {
        (GROUP_CLAIMS, GROUPS_SCHEMA, GROUP_RETRY)
    }

    /// The call for one file: its claims, numbered from 1 in the order
    /// given. Source text goes along since input costs little, and notes
    /// alone leave out context.
    pub fn prompt(&self, path: &str, claims: &[Listed]) -> Prompt {
        let mut user = format!("File: {path}\nClaims:");
        for (index, claim) in claims.iter().enumerate() {
            // One line per claim, whatever line breaks its source text has.
            let quote = claim.quote.split_whitespace().collect::<Vec<_>>().join(" ");
            user.push_str(&format!(
                "\n{} [{}] {} | {quote}",
                index + 1,
                claim.kind,
                claim.text
            ));
        }
        Prompt {
            system: Some(self.system.clone()),
            user,
            schema: Some(self.schema.clone()),
        }
    }

    /// The call for a file again, after a reply with problems.
    pub fn retry(&self, path: &str, claims: &[Listed], problems: &[String]) -> Prompt {
        let mut prompt = self.prompt(path, claims);
        with_problems(&mut prompt.user, GROUP_RETRY, problems);
        prompt
    }
}

/// A section as prompts need it: its lines, the line number of the first,
/// and its passages.
pub struct Section<'a> {
    pub lines: &'a [&'a str],
    pub first_line: usize,
    pub passages: &'a [Passage],
}

/// Pass 1: atomic claims from one section per call.
pub struct Extraction {
    system: String,
    /// The JSON Schema of replies: a list of claims.
    schema: Value,
    /// The same hash as `cat prompts/extract_claims.md
    /// prompts/claims_schema.json prompts/extract_claims_retry.md | git
    /// hash-object --stdin`.
    pub version: String,
}

impl Extraction {
    pub fn load() -> Result<Self, String> {
        let schema: Value = serde_json::from_str(CLAIMS_SCHEMA)
            .map_err(|err| format!("prompts/claims_schema.json: {err}"))?;
        let version =
            git::hash_object(format!("{EXTRACT_CLAIMS}{CLAIMS_SCHEMA}{RETRY}").as_bytes())?;
        Ok(Extraction {
            system: EXTRACT_CLAIMS.trim_end().to_owned(),
            schema,
            version,
        })
    }

    /// The wording of this version: the system prompt, the reply schema and
    /// the retry prompt.
    pub fn wording(&self) -> (&'static str, &'static str, &'static str) {
        (EXTRACT_CLAIMS, CLAIMS_SCHEMA, RETRY)
    }

    /// The call for one section: the file path and heading path as context,
    /// then the section with its passages numbered.
    pub fn prompt(&self, path: &str, headings: &str, section: &Section) -> Prompt {
        let numbered = passages::numbered(section.lines, section.first_line, section.passages);
        Prompt {
            system: Some(self.system.clone()),
            user: format!("File: {path}\nHeadings: {headings}\n<section>\n{numbered}\n</section>"),
            schema: Some(self.schema.clone()),
        }
    }

    /// The call for a section again, after a reply with problems: the same
    /// message, then the problems.
    pub fn retry(
        &self,
        path: &str,
        headings: &str,
        section: &Section,
        problems: &[String],
    ) -> Prompt {
        let mut prompt = self.prompt(path, headings, section);
        with_problems(&mut prompt.user, RETRY, problems);
        prompt
    }
}
