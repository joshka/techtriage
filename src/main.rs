mod database;
mod extensions;
mod models;

use std::fs::File;
use std::path::PathBuf;

use log::info;
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode, WriteLogger};

use database::Database;
use extensions::ExtensionManager;

pub struct Context {
    override_mode: Option<Override>,
}

#[derive(Debug, PartialEq, Eq)]
enum Override {
    Load,
    Handle,
}

impl Context {
    fn new(auto_reload: bool, auto_handle: bool) -> Self {
        Self {
            override_mode: match (auto_reload, auto_handle) {
                (false, false) => None,
                (true, false) => Some(Override::Load),
                (false, true) => Some(Override::Handle),
                // * Clap should prevent this from ever happening
                _ => panic!("Auto-reload and auto-handle are mutually exclusive."),
            },
        }
    }

    /// Creates a context with auto-reload enabled.
    #[cfg(test)]
    fn auto_reload() -> Self {
        Self {
            override_mode: Some(Override::Load),
        }
    }

    /// Creates a context with auto-handle enabled.
    #[cfg(test)]
    fn auto_handle() -> Self {
        Self {
            override_mode: Some(Override::Handle),
        }
    }

    /// Creates a context with no overrides enabled.
    #[cfg(test)]
    fn no_override() -> Self {
        Self {
            override_mode: None,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = get_args();

    let verbose = *args.get_one::<bool>("verbose").unwrap();
    let log_file = args.get_one::<std::path::PathBuf>("log file");
    let auto_reload = *args.get_one::<bool>("auto reload").unwrap();
    let auto_handle = *args.get_one::<bool>("auto handle").unwrap();

    let ctx = Context::new(auto_reload, auto_handle);

    start_logger(verbose, log_file).unwrap();

    info!("TechTriage v{}", env!("CARGO_PKG_VERSION"));
    info!("Starting server...");

    let db = Database::connect().await;

    db.setup_tables().await?;
    db.add_builtins().await?;

    let manager = ExtensionManager::new(&ctx)?;
    manager.load_extensions(&db).await?;

    stop(0);
}

/// Parses the provided CLI arguments into a usable format.
fn get_args() -> clap::ArgMatches {
    use clap::{value_parser, Arg, ArgAction, Command};
    Command::new("techtriage")
        .bin_name("techtriage")
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("Enable verbose output for debugging purposes."),
        )
        .arg(
            Arg::new("log file")
                .short('l')
                .long("log-file")
                .value_parser(value_parser!(std::path::PathBuf))
                .help("Write logs to the specified file instead of stderr.",),
        )
        .arg(
            Arg::new("auto reload")
                .long("auto-reload")
                .action(ArgAction::SetTrue)
                .conflicts_with("auto handle")
                .help(
                    "Force all extensions to be reloaded on startup, even if their version has not \
                    changed. This is useful for development and testing of extensions.",
                ),
        )
        .arg(
            Arg::new("auto handle")
                .long("auto-handle")
                .action(ArgAction::SetTrue)
                .conflicts_with("auto reload")
                .help(
                    "Enable the server to automatically handle extension conflicts instead of \
                    asking the user to resolve them. This could result in unexpected behavior.",
                ),
        )
        .get_matches()
}

/// Initializes either a terminal or file logger, depending on the provided configuration.
fn start_logger(verbose: bool, path: Option<&PathBuf>) -> anyhow::Result<()> {
    match path {
        Some(path) => {
            WriteLogger::init(
                match verbose {
                    true => LevelFilter::Debug,
                    false => LevelFilter::Info,
                },
                Config::default(),
                // ? Should the log file be overwritten automatically?
                File::create(path)?,
            )?;
        }
        None => {
            TermLogger::init(
                match verbose {
                    true => LevelFilter::Debug,
                    false => LevelFilter::Info,
                },
                Config::default(),
                TerminalMode::Stderr,
                ColorChoice::Auto,
            )?;
        }
    }

    Ok(())
}

/// Exits the program with a friendly log message instead of an ugly panic message.
fn stop(code: i32) -> ! {
    info!("Stopping server...");
    std::process::exit(code);
}
