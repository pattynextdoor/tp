use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::db;
use crate::nav::frecency;
use crate::nav::waypoints;
use crate::project;
use crate::shell;

/// tp — Teleport anywhere in your codebase.
///
/// A blazing-fast, project-aware directory navigator for the terminal.
#[derive(Parser, Debug)]
#[command(name = "tp", version, about)]
pub struct Cli {
    /// Navigate to a directory matching this query
    #[arg(trailing_var_arg = true)]
    pub query: Vec<String>,

    /// Interactive picker mode
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    /// Project-scoped search
    #[arg(short = 'p', long = "project")]
    pub project_scoped: bool,

    /// Create a waypoint (bookmark) for a directory
    #[arg(long = "mark", value_name = "NAME", num_args = 1..=2)]
    pub mark: Option<Vec<String>>,

    /// Remove a waypoint
    #[arg(long = "unmark", value_name = "NAME")]
    pub unmark: Option<String>,

    /// List all waypoints
    #[arg(long = "waypoints")]
    pub waypoints: bool,

    /// Configure AI API key
    #[arg(long = "setup-ai")]
    pub setup_ai: bool,

    /// AI session recall (requires 'ai' feature)
    #[arg(long = "recall")]
    pub recall: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate shell initialization code
    Init {
        /// Shell to generate init for (bash, zsh, fish, powershell, nushell, elvish)
        shell: String,

        /// Custom command name (default: tp)
        #[arg(long, default_value = "tp")]
        cmd: String,
    },

    /// Import navigation data from another tool
    Import {
        /// Tool to import from (zoxide, z, autojump)
        #[arg(long = "from")]
        from: String,

        /// Path to the database file (auto-detected if omitted)
        path: Option<String>,
    },

    /// Record a directory visit (called by shell hooks)
    Add {
        /// The directory path to record
        path: String,
    },

    /// Cloud sync (Pro feature, stub)
    Sync,
}

/// Entry point: parse CLI args and dispatch to the appropriate handler.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands first
    if let Some(cmd) = &cli.command {
        return match cmd {
            Commands::Init { shell, cmd } => {
                let code = shell::generate_init(shell, cmd)?;
                print!("{}", code);
                Ok(())
            }
            Commands::Import { from, path } => {
                eprintln!("Import from '{}' is not yet implemented.", from);
                if let Some(p) = path {
                    eprintln!("(path: {})", p);
                }
                Ok(())
            }
            Commands::Add { path } => {
                let conn = db::open()?;
                let project_root = project::detect_project_root(path);
                frecency::record_visit(&conn, path, project_root.as_deref())?;
                Ok(())
            }
            Commands::Sync => {
                eprintln!("Cloud sync is a Pro feature and is not yet implemented.");
                Ok(())
            }
        };
    }

    // Handle flags
    if cli.waypoints {
        let conn = db::open()?;
        return waypoints::list_waypoints(&conn);
    }

    if let Some(ref mark_args) = cli.mark {
        let conn = db::open()?;
        let name = &mark_args[0];
        let path = if mark_args.len() > 1 {
            mark_args[1].clone()
        } else {
            std::env::current_dir()?
                .to_string_lossy()
                .to_string()
        };
        return waypoints::add_waypoint(&conn, name, &path);
    }

    if let Some(ref name) = cli.unmark {
        let conn = db::open()?;
        return waypoints::remove_waypoint(&conn, name);
    }

    if cli.setup_ai {
        #[cfg(feature = "ai")]
        {
            crate::ai::setup_key()?;
            return Ok(());
        }
        #[cfg(not(feature = "ai"))]
        {
            eprintln!("AI features are not enabled. Rebuild with --features ai");
            return Ok(());
        }
    }

    if cli.recall {
        #[cfg(feature = "ai")]
        {
            crate::ai::recall::session_recall()?;
            return Ok(());
        }
        #[cfg(not(feature = "ai"))]
        {
            eprintln!("AI features are not enabled. Rebuild with --features ai");
            return Ok(());
        }
    }

    // Main navigation flow
    if cli.query.is_empty() {
        eprintln!("Usage: tp <query> — teleport to a directory");
        eprintln!("       tp --help  — show all options");
        return Ok(());
    }

    let conn = db::open()?;
    match crate::nav::navigate(&conn, &cli.query)? {
        Some(result) => {
            // Print path to stdout — the shell wrapper captures this and does `cd`
            println!("{}", result.path);
        }
        None => {
            eprintln!("No match found for: {}", cli.query.join(" "));
            std::process::exit(1);
        }
    }

    Ok(())
}
