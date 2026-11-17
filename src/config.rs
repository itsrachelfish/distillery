//! Distillery's settings, read from a TOML file laid out like
//! `distillery.example.toml`. The API key is never a setting: it comes only
//! from the `ANTHROPIC_API_KEY` environment variable.

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

        // The API key comes only from the environment.
        let text = format!("{MINIMAL}[model]\napi_key = \"secret\"\n");
        assert!(Config::parse(&text).is_err());
    }

    #[test]
    fn explains_a_missing_file() {
        let err = Config::load(Path::new("no-such-folder/distillery.toml")).unwrap_err();
        assert!(err.contains("distillery.example.toml"), "{err}");
    }
}
