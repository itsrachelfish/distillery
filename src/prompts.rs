//! What models are asked: the prompts and reply schemas in `prompts/`,
//! tracked in git and built into the binary.
//!
//! The prompt version is the hash git gives the files, so records say which
//! wording produced them, and records from older wording show as needing
//! another run.

use serde_json::Value;

use crate::git;
use crate::model::Prompt;

const EXTRACT_CLAIMS: &str = include_str!("../prompts/extract_claims.md");
const CLAIMS_SCHEMA: &str = include_str!("../prompts/claims_schema.json");
const RETRY: &str = include_str!("../prompts/extract_claims_retry.md");
/// Where the retry prompt lists the problems.
const PROBLEMS: &str = "{problems}";

/// What usage records call the calls that extract claims.
pub const EXTRACT_OPERATION: &str = "extract_claims";

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
    /// then the section as written.
    pub fn prompt(&self, path: &str, headings: &str, section: &str) -> Prompt {
        Prompt {
            system: Some(self.system.clone()),
            user: format!("File: {path}\nHeadings: {headings}\n<section>\n{section}\n</section>"),
            schema: Some(self.schema.clone()),
        }
    }

    /// The call for a section again, after a reply with problems: the same
    /// message, then the problems.
    pub fn retry(&self, path: &str, headings: &str, section: &str, problems: &[String]) -> Prompt {
        let list: Vec<String> = problems
            .iter()
            .map(|problem| format!("- {problem}"))
            .collect();
        let mut prompt = self.prompt(path, headings, section);
        prompt.user.push_str("\n\n");
        prompt.user.push_str(
            RETRY
                .trim_end()
                .replace(PROBLEMS, &list.join("\n"))
                .as_str(),
        );
        prompt
    }
}
