use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::io;
use std::path::PathBuf;

/// Content-addressed CSV store with collection-aware commands
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Repository root folder
    #[arg(long = "repo", default_value = ricotta::DEFAULT_REPO_ROOT)]
    repo: PathBuf,

    /// Enable debug logs for repository file reads and writes
    #[arg(long = "debug")]
    debug: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a collection schema from ordered headers
    Create {
        /// Collection name
        collection: String,
        /// Ordered header list for this collection
        headers: Vec<String>,
    },
    /// Ingest a CSV into the repository
    Set {
        /// Dataset key `<collection>/<dataset>`
        name: String,
        /// Optional input CSV path. If omitted, read from stdin.
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
    },
    /// List collections or datasets in one collection
    List {
        /// Optional collection name
        collection: Option<String>,
    },
    /// Reconstruct a stored dataset as CSV
    Get {
        /// Dataset key `<collection>/<dataset>`
        name: String,
        /// Optional output CSV path. If omitted, write to stdout.
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let debug = cli.debug;
    if debug {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .format_timestamp(None)
            .try_init()
            .ok();
    }
    let repo = ricotta::Repo::new(cli.repo);

    match cli.command {
        Command::Create {
            collection,
            headers,
        } => {
            ricotta::create_collection_in_repo(&repo, &collection, headers)?;
        }
        Command::Set { name, file } => match file {
            Some(path) => {
                if debug {
                    log::debug!("read: {}", path.display());
                }
                let file = fs::File::open(&path)?;
                let _ = ricotta::put_from_reader_in_repo(&repo, file, &name)?;
            }
            None => {
                let _ = ricotta::put_from_reader_in_repo(
                    &repo,
                    io::stdin().lock(),
                    &name,
                )?;
            }
        },
        Command::List { collection } => {
            let names = match collection {
                Some(collection) => {
                    ricotta::list_datasets_in_collection_in_repo(
                        &repo,
                        &collection,
                    )?
                }
                None => ricotta::list_collections_in_repo(&repo)?,
            };
            for name in &names {
                println!("{name}");
            }
        }
        Command::Get { name, file } => match file {
            Some(path) => {
                if debug {
                    log::debug!("write: {}", path.display());
                }
                let file = fs::File::create(&path)?;
                ricotta::get_to_writer_in_repo(&repo, &name, file)?;
            }
            None => {
                let stdout = io::stdout();
                let lock = stdout.lock();
                ricotta::get_to_writer_in_repo(&repo, &name, lock)?;
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_create_command() {
        let cli = Cli::try_parse_from([
            "ricotta",
            "--debug",
            "--repo",
            ".risotto-dev",
            "create",
            "temperature",
            "city",
            "temp",
        ])
        .unwrap();

        match cli.command {
            Command::Create {
                collection,
                headers,
            } => {
                assert!(cli.debug);
                assert_eq!(cli.repo, PathBuf::from(".risotto-dev"));
                assert_eq!(collection, "temperature");
                assert_eq!(headers, vec!["city", "temp"]);
            }
            _ => panic!("expected create command"),
        }
    }

    #[test]
    fn parse_set_and_get_commands() {
        let set_cli = Cli::try_parse_from([
            "ricotta",
            "set",
            "temperature/london",
            "--file",
            "input.csv",
        ])
        .unwrap();
        match set_cli.command {
            Command::Set { name, file } => {
                assert_eq!(name, "temperature/london");
                assert_eq!(file, Some(PathBuf::from("input.csv")));
            }
            _ => panic!("expected set command"),
        }

        let get_cli = Cli::try_parse_from([
            "ricotta",
            "get",
            "temperature/london",
            "-f",
            "output.csv",
        ])
        .unwrap();
        match get_cli.command {
            Command::Get { name, file } => {
                assert_eq!(name, "temperature/london");
                assert_eq!(file, Some(PathBuf::from("output.csv")));
            }
            _ => panic!("expected get command"),
        }
    }

    #[test]
    fn parse_list_command_variants() {
        let list_collections =
            Cli::try_parse_from(["ricotta", "list"]).unwrap();
        match list_collections.command {
            Command::List { collection } => {
                assert_eq!(collection, None);
            }
            _ => panic!("expected list command"),
        }

        let list_datasets =
            Cli::try_parse_from(["ricotta", "list", "temperature"]).unwrap();
        match list_datasets.command {
            Command::List { collection } => {
                assert_eq!(collection.as_deref(), Some("temperature"));
            }
            _ => panic!("expected list command"),
        }
    }
}
