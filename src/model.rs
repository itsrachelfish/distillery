//! Model access, apart from any one provider.
//!
//! Distillery decides what to send and checks what comes back. Providers
//! only carry requests to an API, so another API can be added without
//! changing the stages that use a model.

/// What a model is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Instructions that hold for the whole call.
    pub system: Option<String>,
    /// The text to work on.
    pub user: String,
}

/// An API that runs models.
pub trait ModelProvider {
    /// The provider's name in usage records, such as `anthropic`.
    fn provider(&self) -> &'static str;

    /// The model that calls go to.
    fn model(&self) -> &str;

    /// How many input tokens a prompt would use. Providers count for free,
    /// so estimates cost nothing.
    fn count_tokens(&self, prompt: &Prompt) -> Result<u64, String>;
}
