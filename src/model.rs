//! Model access, apart from any one provider.
//!
//! Distillery decides what to send and checks what comes back. Providers
//! only carry requests to an API, so another API can be added without
//! changing the stages that use a model.

use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::progress::{Progress, counted};

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

/// What a call used, as the provider reports it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    /// Everything billed as output, thinking included.
    pub output_tokens: u64,
    /// The part of the output tokens spent on thinking.
    pub thinking_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
}

/// A model's reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// The reply's text, without any thinking.
    pub text: String,
    /// Why the model stopped, such as `end_turn` for a complete reply,
    /// `max_tokens` for a reply cut off at the limit, or `refusal`.
    pub stop_reason: String,
    pub usage: Usage,
}

impl Reply {
    /// Whether the model finished the reply itself. Only complete replies
    /// are held to a prompt's schema.
    pub fn complete(&self) -> bool {
        self.stop_reason == "end_turn"
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

    /// Runs the model on a prompt. This is the call that costs money.
    fn reply(&self, prompt: &Prompt) -> Result<Reply, ModelError>;
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

/// Makes one call, attempting again after rate limits and server errors
/// while showing the wait. `record` hears of every attempt with how long it
/// took, so each one reaches the usage records. `place` says what the call
/// is for, and `done` is the progress so far.
pub fn attempt<T>(
    progress: &mut Progress,
    done: u64,
    place: &str,
    mut call: impl FnMut() -> Result<T, ModelError>,
    mut record: impl FnMut(Duration, Result<&T, &ModelError>) -> Result<(), String>,
) -> Result<T, String> {
    let mut attempt = 1;
    loop {
        let started = Instant::now();
        let reply = call();
        record(started.elapsed(), reply.as_ref())?;
        let error = match reply {
            Ok(reply) => return Ok(reply),
            Err(error) => error,
        };
        let Some(wait) = retry_wait(attempt, &error) else {
            return Err(format!(
                "{place} failed after {}: {error}",
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
                    "{place}, attempt {attempt} of {MAX_ATTEMPTS} in {}s: {error}",
                    left.as_secs() + 1
                )
            });
            thread::sleep(left.min(Duration::from_secs(1)));
        }
    }
}
