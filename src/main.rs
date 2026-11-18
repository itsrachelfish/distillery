mod anthropic;
mod config;
mod db;
mod estimate;
mod extract;
mod git;
mod group;
mod model;
mod passages;
mod progress;
mod prompts;
mod sections;
mod snapshot;
#[cfg(test)]
mod testing;
mod web;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::progress::{Progress, counted};

/// Distills docs into key facts, cited by source, confirmed by humans.
///
/// Without a command, prints the settings in use and creates or upgrades
/// the database.
#[derive(Debug, Parser)]
#[command(name = "distillery", version)]
struct Cli {
    /// Settings file, laid out like distillery.example.toml
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        default_value = "distillery.toml"
    )]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the web interface on this computer only
    Serve,
    /// Clone a repository, or fetch a known one, record its commit and list
    /// its Markdown files
    Snapshot {
        /// An https or ssh URL, such as https://github.com/wetfish/server
        url: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> db::Result<()> {
    let config = Config::load(&cli.config)?;
    let database = &config.storage.database;
    let conn =
        db::open(database).map_err(|err| format!("opening {}: {err}", database.display()))?;

    match cli.command {
        None => {
            println!("Settings: {}", cli.config.display());
            println!("Reviewer: {}", config.review.reviewer);
            println!("Model: {}", config.model.name);
            println!(
                "API key: {}",
                if config.model.api_key.is_empty() {
                    "not set"
                } else {
                    "set"
                }
            );
            println!(
                "Database: {} (schema version {})",
                database.display(),
                db::schema_version(&conn)?
            );
            println!("Clone folder: {}", config.storage.clone_dir.display());
            println!("Server port: {}", config.server.port);
        }
        Some(Command::Serve) => {
            // Pages open the database themselves, one connection each.
            drop(conn);
            web::run(&config)?;
        }
        Some(Command::Snapshot { url }) => {
            let mut progress = Progress::to_stderr();
            let found = snapshot::run(&conn, &config.storage.clone_dir, &url, &mut progress)?;
            drop(progress);
            println!("Clone: {}", found.clone.display());
            println!(
                "Commit: {} ({})",
                found.commit,
                if found.new_commit {
                    "new"
                } else {
                    "already recorded"
                }
            );
            println!(
                "{}:",
                counted(found.files.len() as u64, "Markdown file", "Markdown files")
            );
            for file in &found.files {
                println!("{file}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_line_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn reads_the_default_settings_file() {
        let cli = Cli::try_parse_from(["distillery"]).unwrap();
        assert_eq!(cli.config, PathBuf::from("distillery.toml"));
        assert!(cli.command.is_none());
    }

    #[test]
    fn takes_the_settings_file_before_or_after_the_command() {
        let url = "https://github.com/wetfish/server";
        for args in [
            ["distillery", "--config", "other.toml", "snapshot", url],
            ["distillery", "snapshot", url, "--config", "other.toml"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!(cli.config, PathBuf::from("other.toml"));
            let Some(Command::Snapshot { url: parsed }) = cli.command else {
                panic!("no snapshot command in {args:?}");
            };
            assert_eq!(parsed, url);
        }
    }

    #[test]
    fn rejects_unknown_commands_and_missing_urls() {
        assert!(Cli::try_parse_from(["distillery", "distill"]).is_err());
        assert!(Cli::try_parse_from(["distillery", "snapshot"]).is_err());
    }
}
