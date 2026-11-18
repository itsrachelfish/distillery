//! The Anthropic Messages API, called over HTTPS.
//!
//! The API key comes from the `model.api_key` setting. It is never written
//! to the database, logs or error messages.

use std::time::Duration;

use serde_json::{Value, json};

use crate::config;
use crate::model::{ModelProvider, Prompt};

const API: &str = "https://api.anthropic.com";
/// The API version every request names.
const VERSION: &str = "2023-06-01";
/// How long one request may take.
const TIMEOUT: Duration = Duration::from_secs(120);

pub struct Anthropic {
    client: reqwest::blocking::Client,
    key: String,
    model: String,
}

impl Anthropic {
    /// A client for the configured model, with the configured API key.
    pub fn new(model: &config::Model) -> Result<Self, String> {
        if model.api_key.is_empty() {
            return Err(
                "model.api_key is not set. Add the key to distillery.toml and restart the \
                 server."
                    .to_owned(),
            );
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("distillery/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|err| format!("Setting up HTTPS: {err}"))?;
        Ok(Anthropic {
            client,
            key: model.api_key.clone(),
            model: model.name.clone(),
        })
    }

    /// Posts a JSON body and returns the JSON reply.
    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        let response = self
            .client
            .post(format!("{API}{path}"))
            .header("x-api-key", self.key.as_str())
            .header("anthropic-version", VERSION)
            .json(body)
            .send()
            .map_err(|err| format!("Reaching the Anthropic API: {err}"))?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|err| format!("Reading the Anthropic API's reply: {err}"))?;
        let reply: Option<Value> = serde_json::from_str(&text).ok();
        if !status.is_success() {
            return Err(api_error(status.as_u16(), reply.as_ref(), &text));
        }
        reply.ok_or_else(|| "The Anthropic API's reply was not JSON.".to_owned())
    }
}

impl ModelProvider for Anthropic {
    fn provider(&self) -> &'static str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn count_tokens(&self, prompt: &Prompt) -> Result<u64, String> {
        let mut body = json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt.user }],
        });
        if let Some(system) = &prompt.system {
            body["system"] = json!(system);
        }
        let reply = self.post("/v1/messages/count_tokens", &body)?;
        reply["input_tokens"]
            .as_u64()
            .ok_or_else(|| "The Anthropic API's token count had no input_tokens.".to_owned())
    }
}

/// How much of a reply that is not an API error an error message quotes.
const QUOTED: usize = 200;

/// An error reply in words: the HTTP status with the API's own error type
/// and message, such as `401 (authentication_error): invalid x-api-key`.
fn api_error(status: u16, reply: Option<&Value>, text: &str) -> String {
    let error = reply.map(|reply| &reply["error"]);
    let kind = error.and_then(|error| error["type"].as_str());
    let message = error.and_then(|error| error["message"].as_str());
    match (kind, message) {
        (Some(kind), Some(message)) => {
            format!("The Anthropic API returned {status} ({kind}): {message}")
        }
        _ => {
            let quoted: String = text.chars().take(QUOTED).collect();
            format!("The Anthropic API returned {status}: {}", quoted.trim())
        }
    }
}
