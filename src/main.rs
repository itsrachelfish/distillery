mod config;
mod db;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::config::Config;

/// The settings file read when none is given. `.gitignore` excludes it.
const DEFAULT_CONFIG: &str = "distillery.toml";

const USAGE: &str = "usage: distillery [CONFIG]

Reads the settings in CONFIG, a TOML file laid out like
distillery.example.toml, creates or upgrades the database, and prints the
settings in use. CONFIG defaults to distillery.toml.";

/// What the command line asks for.
#[derive(Debug, PartialEq)]
enum Args {
    Run(PathBuf),
    Help,
    Version,
}

fn main() -> ExitCode {
    let path = match parse_args(std::env::args().skip(1)) {
        Ok(Args::Run(path)) => path,
        Ok(Args::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Args::Version) => {
            println!("distillery {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(err) => {
            eprintln!("error: {err}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let config = match Config::load(&path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let database = &config.storage.database;
    let version = match db::open(database).and_then(|conn| db::schema_version(&conn)) {
        Ok(version) => version,
        Err(err) => {
            eprintln!("error: opening {}: {err}", database.display());
            return ExitCode::FAILURE;
        }
    };

    println!("Settings: {}", path.display());
    println!("Reviewer: {}", config.review.reviewer);
    println!("Model: {}", config.model.name);
    println!(
        "Database: {} (schema version {version})",
        database.display()
    );
    println!("Clone folder: {}", config.storage.clone_dir.display());
    println!("Server port: {}", config.server.port);
    ExitCode::SUCCESS
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let Some(first) = args.next() else {
        return Ok(Args::Run(PathBuf::from(DEFAULT_CONFIG)));
    };
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument '{extra}'"));
    }
    match first.as_str() {
        "-h" | "--help" => Ok(Args::Help),
        "-V" | "--version" => Ok(Args::Version),
        flag if flag.starts_with('-') => Err(format!("unknown option '{flag}'")),
        _ => Ok(Args::Run(PathBuf::from(first))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn reads_the_default_settings_file() {
        assert_eq!(parse(&[]), Ok(Args::Run(PathBuf::from("distillery.toml"))));
        assert_eq!(
            parse(&["other.toml"]),
            Ok(Args::Run(PathBuf::from("other.toml")))
        );
    }

    #[test]
    fn answers_help_and_version() {
        assert_eq!(parse(&["-h"]), Ok(Args::Help));
        assert_eq!(parse(&["--help"]), Ok(Args::Help));
        assert_eq!(parse(&["-V"]), Ok(Args::Version));
        assert_eq!(parse(&["--version"]), Ok(Args::Version));
    }

    #[test]
    fn rejects_unknown_options_and_extra_arguments() {
        assert!(parse(&["--db"]).is_err());
        assert!(parse(&["a.toml", "b.toml"]).is_err());
    }
}
