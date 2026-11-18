//! Distillery's settings, read from a TOML file laid out like
//! `distillery.example.toml`. The file holds the model API key, so git
//! ignores `distillery.toml`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub server: Server,
    pub review: Review,
    #[serde(default)]
    pub model: Model,
    #[serde(default)]
    pub storage: Storage,
}

/// The web interface, served on 127.0.0.1 only.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    #[serde(default = "default_port")]
    pub port: u16,
}

/// Who reviews. Decisions have no default author, so this is required.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    /// Reviewer recorded on every decision made through this server.
    pub reviewer: String,
}

/// The model for new runs. Each run records its model.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    #[serde(default = "default_model")]
    pub name: String,
    /// The API key for the model's provider. Empty when not set. Never
    /// shown on pages or written to the database or logs.
    #[serde(default)]
    pub api_key: String,
    /// US dollars per million input tokens.
    #[serde(default = "default_input_price")]
    pub input_price: f64,
    /// US dollars per million output tokens.
    #[serde(default = "default_output_price")]
    pub output_price: f64,
    /// Runs stop once their recorded cost passes this many US dollars.
    #[serde(default = "default_max_run_cost")]
    pub max_run_cost: f64,
    /// The most output tokens one call may use, thinking included.
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: u32,
}

impl Model {
    /// The cost of input tokens at the configured price, in millionths of a
    /// US dollar. Prices are dollars per million tokens, so cost is tokens
    /// times price.
    pub fn input_cost_micros(&self, tokens: i64) -> i64 {
        (tokens as f64 * self.input_price).round() as i64
    }

    /// The cost of output tokens, in millionths of a US dollar.
    pub fn output_cost_micros(&self, tokens: i64) -> i64 {
        (tokens as f64 * self.output_price).round() as i64
    }

    /// The run cost limit, in millionths of a US dollar.
    pub fn max_run_cost_micros(&self) -> i64 {
        (self.max_run_cost * 1_000_000.0).round() as i64
    }
}

/// Where Distillery keeps its data.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    /// SQLite database, created when missing.
    #[serde(default = "default_database")]
    pub database: PathBuf,
    /// Folder for repository clones, one subfolder per repository.
    #[serde(default = "default_clone_dir")]
    pub clone_dir: PathBuf,
}

fn default_port() -> u16 {
    8080
}

fn default_model() -> String {
    "claude-opus-5-5".to_owned()
}

fn default_input_price() -> f64 {
    4.0
}

fn default_output_price() -> f64 {
    20.0
}

fn default_max_output_tokens() -> u32 {
    8192
}

fn default_max_run_cost() -> f64 {
    2.0
}

fn default_database() -> PathBuf {
    PathBuf::from("distillery.sqlite")
}

fn default_clone_dir() -> PathBuf {
    PathBuf::from("repos")
}

impl Default for Server {
    fn default() -> Self {
        Server {
            port: default_port(),
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Model {
            name: default_model(),
            api_key: String::new(),
            input_price: default_input_price(),
            output_price: default_output_price(),
            max_run_cost: default_max_run_cost(),
            max_output_tokens: default_max_output_tokens(),
        }
    }
}

impl Default for Storage {
    fn default() -> Self {
        Storage {
            database: default_database(),
            clone_dir: default_clone_dir(),
        }
    }
}

impl Config {
    /// Reads and checks the settings file.
    pub fn load(path: &Path) -> Result<Config, String> {
        let text = fs::read_to_string(path).map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                format!(
                    "{} not found. Copy distillery.example.toml to distillery.toml and set \
                     review.reviewer",
                    path.display()
                )
            } else {
                format!("reading {}: {err}", path.display())
            }
        })?;
        Config::parse(&text).map_err(|err| format!("{}: {err}", path.display()))
    }

    /// The settings in `distillery.example.toml`, for tests.
    #[cfg(test)]
    pub fn example() -> Config {
        Config::parse(include_str!("../distillery.example.toml")).unwrap()
    }

    fn parse(text: &str) -> Result<Config, String> {
        let config: Config = toml::from_str(text).map_err(|err| err.to_string())?;
        config.check()?;
        Ok(config)
    }

    /// Catches unusable settings before anything runs.
    fn check(&self) -> Result<(), String> {
        if self.server.port == 0 {
            return Err("server.port must be a port number from 1 to 65535".to_owned());
        }

        let reviewer = &self.review.reviewer;
        if reviewer.trim().is_empty() || reviewer.contains(char::is_control) {
            return Err("review.reviewer must be a name on one line".to_owned());
        }
        if reviewer.trim() != reviewer {
            return Err("review.reviewer must not start or end with spaces".to_owned());
        }

        let model = &self.model.name;
        if model.is_empty() || model.contains(|c: char| c.is_whitespace() || c.is_control()) {
            return Err("model.name must be a model name without spaces".to_owned());
        }
        if self
            .model
            .api_key
            .contains(|c: char| c.is_whitespace() || c.is_control())
        {
            return Err("model.api_key must be the key alone, without spaces".to_owned());
        }
        for (setting, amount) in [
            ("model.input_price", self.model.input_price),
            ("model.output_price", self.model.output_price),
            ("model.max_run_cost", self.model.max_run_cost),
        ] {
            if !amount.is_finite() || amount < 0.0 {
                return Err(format!(
                    "{setting} must be an amount in US dollars, zero or more"
                ));
            }
        }

        if self.model.max_output_tokens == 0 {
            return Err("model.max_output_tokens must be 1 or more".to_owned());
        }

        if self.storage.database.as_os_str().is_empty() {
            return Err("storage.database must not be empty".to_owned());
        }
        if self.storage.clone_dir.as_os_str().is_empty() {
            return Err("storage.clone_dir must not be empty".to_owned());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "[review]\nreviewer = \"Rachel Fish\"\n";

    #[test]
    fn example_file_is_valid() {
        let config = Config::example();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.review.reviewer, "Rachel Fish");
        assert_eq!(config.model.name, "claude-opus-5-5");
        assert_eq!(config.storage.database, PathBuf::from("distillery.sqlite"));
        assert_eq!(config.storage.clone_dir, PathBuf::from("repos"));
    }

    #[test]
    fn fills_in_defaults() {
        let config = Config::parse(MINIMAL).unwrap();
        assert_eq!(config, Config::example());
    }

    #[test]
    fn fills_in_defaults_within_sections() {
        let text = format!("{MINIMAL}[storage]\ndatabase = \"other.sqlite\"\n");
        let config = Config::parse(&text).unwrap();
        assert_eq!(config.storage.database, PathBuf::from("other.sqlite"));
        assert_eq!(config.storage.clone_dir, PathBuf::from("repos"));
    }

    #[test]
    fn requires_a_reviewer() {
        let err = Config::parse("").unwrap_err();
        assert!(err.contains("review"), "{err}");

        for reviewer in ["", "   ", " Rachel", "Rachel\\nFish"] {
            let text = format!("[review]\nreviewer = \"{reviewer}\"\n");
            let err = Config::parse(&text).unwrap_err();
            assert!(err.contains("review.reviewer"), "{reviewer:?}: {err}");
        }
    }

    #[test]
    fn rejects_unusable_settings() {
        for (section, setting, value) in [
            ("server", "port", "0"),
            ("server", "port", "70000"),
            ("model", "name", "\"\""),
            ("model", "name", "\"claude opus\""),
            ("storage", "database", "\"\""),
            ("storage", "clone_dir", "\"\""),
        ] {
            let text = format!("{MINIMAL}[{section}]\n{setting} = {value}\n");
            assert!(
                Config::parse(&text).is_err(),
                "accepted {section}.{setting} = {value}"
            );
        }
    }

    #[test]
    fn rejects_unknown_settings() {
        // Repositories are added through the web interface, never listed here.
        let text = format!("{MINIMAL}[storage]\nrepos = [\"https://example.net/a.git\"]\n");
        assert!(Config::parse(&text).is_err());

        let text = format!("{MINIMAL}[model]\nkey = \"secret\"\n");
        assert!(Config::parse(&text).is_err());
    }

    #[test]
    fn reads_the_api_key() {
        assert_eq!(Config::parse(MINIMAL).unwrap().model.api_key, "");

        let text = format!("{MINIMAL}[model]\napi_key = \"secret\"\n");
        assert_eq!(Config::parse(&text).unwrap().model.api_key, "secret");

        let text = format!("{MINIMAL}[model]\napi_key = \"two words\"\n");
        assert!(Config::parse(&text).is_err());
    }

    #[test]
    fn explains_a_missing_file() {
        let err = Config::load(Path::new("no-such-folder/distillery.toml")).unwrap_err();
        assert!(err.contains("distillery.example.toml"), "{err}");
    }
}
