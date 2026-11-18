//! Model access, apart from any one provider.
//!
//! Distillery decides what to send and checks what comes back. Providers
//! only carry requests to an API, so another API can be added without
//! changing the stages that use a model.

use std::fmt;
use std::time::Duration;

use serde_json::Value;

/// What a model is asked.
#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    /// Instructions that hold for the whole call.
    pub system: Option<String>,
    /// The text to work on.
    pub user: String,
    /// The JSON Schema that complete replies are held to, when replies are
    /// data.
    pub schema: Option<Value>,
}

/// A failed call to a model API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError {
    /// The HTTP status of the reply. `None` when no reply arrived.
    pub status: Option<u16>,
    /// How long the API asked callers to wait before the next attempt.
    pub retry_after: Option<Duration>,
    pub message: String,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// An API that runs models.
pub trait ModelProvider {
    /// The provider's name in usage records, such as `anthropic`.
    fn provider(&self) -> &'static str;

    /// The model that calls go to.
    fn model(&self) -> &str;

    /// How many input tokens a prompt would use. Providers count for free,
    /// so estimates cost nothing.
    fn count_tokens(&self, prompt: &Prompt) -> Result<u64, ModelError>;
}

/// How many times one call is attempted before its error stands.
pub const MAX_ATTEMPTS: u32 = 4;
/// The wait after a first failed attempt. Each later wait doubles.
const FIRST_WAIT: Duration = Duration::from_secs(2);
/// The longest wait before another attempt, whatever the API asked for.
const LONGEST_WAIT: Duration = Duration::from_secs(60);

/// How long to wait before attempting a call again, after its attempt
/// number `attempt`, counting from 1, failed with `error`. `None` when the
/// error stands: other attempts would fail the same way, or enough were made.
///
/// Rate limits (429), server errors (5xx, with 529 for an overloaded API)
/// and calls with no reply are attempted again. The API's own `retry-after`
/// is honored when it is longer than the doubling wait.
pub fn retry_wait(attempt: u32, error: &ModelError) -> Option<Duration> {
    if attempt >= MAX_ATTEMPTS {
        return None;
    }
    let again = match error.status {
        None => true,
        Some(status) => status == 429 || (500..=599).contains(&status),
    };
    if !again {
        return None;
    }
    let doubling = FIRST_WAIT * 2u32.pow(attempt.saturating_sub(1));
    let wait = error
        .retry_after
        .map_or(doubling, |asked| asked.max(doubling));
    Some(wait.min(LONGEST_WAIT))
}
