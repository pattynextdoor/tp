use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use rusqlite::Connection;

use crate::db;
use crate::import;
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

    /// Output completion candidates for the given prefix (used by shell completion scripts)
    #[arg(long = "complete", hide = true)]
    pub complete: Option<String>,

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

        /// Force-run bootstrap to (re-)seed the database from shell history, zoxide, and project discovery
        #[arg(long)]
        bootstrap: bool,
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

    /// List top directories by frecency score
    #[command(name = "ls", alias = "list")]
    Ls {
        /// Number of entries to show (default: 20)
        #[arg(short = 'n', long, default_value = "20")]
        count: usize,
    },

    /// Jump back in navigation history
    Back {
        /// How many steps back (default: 1)
        #[arg(default_value = "1")]
        steps: usize,
    },

    /// Show navigation statistics dashboard
    #[cfg(feature = "tui")]
    Stats,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },

    /// Remove a directory from the database
    Remove {
        /// Path to remove
        path: String,
    },

    /// Print matching directories (for scripting)
    Query {
        /// Search terms
        #[arg(required = true)]
        terms: Vec<String>,

        /// Show scores alongside paths
        #[arg(short, long)]
        score: bool,
    },

    /// Diagnose configuration issues
    Doctor,

    /// AI: build a semantic index of a project (coming soon)
    Index {
        /// Path to the project to index (defaults to current directory)
        path: Option<String>,
    },

    /// AI: extract workflow patterns from navigation history (coming soon)
    Analyze,

    /// Suggest waypoint names for frequently visited directories
    Suggest {
        /// Interactively apply suggested waypoints
        #[arg(long)]
        apply: bool,

        /// Use AI to generate creative waypoint names
        #[arg(long)]
        ai: bool,

        /// Number of suggestions to show
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
    },
}

/// Get the current git branch for a directory.
fn get_git_branch(path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", path, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}

/// Find the closest matching directory name for a "did you mean?" suggestion.
fn suggest_closest(conn: &Connection, query: &str) -> Result<String> {
    let candidates = frecency::query_all(conn, 50)?;
    let query_lower = query.to_lowercase();

    let mut best: Option<(usize, String)> = None;

    for c in &candidates {
        let basename = c
            .path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&c.path)
            .to_lowercase();

        let dist = strsim::damerau_levenshtein(&query_lower, &basename);

        // Only suggest if reasonably close (distance <= half the query length + 1)
        let max_dist = (query_lower.len() / 2) + 1;
        if dist <= max_dist && (best.is_none() || dist < best.as_ref().unwrap().0) {
            best = Some((dist, basename));
        }
    }

    best.map(|(_, name)| name)
        .ok_or_else(|| anyhow::anyhow!("no suggestion"))
}

/// Print dynamic completion candidates for the given prefix.
/// Outputs waypoint names (prefixed with !) and top directory basenames.
fn print_completions(conn: &Connection, prefix: &str) -> Result<()> {
    // Waypoint completions for `!` prefix
    if let Some(wp_prefix) = prefix.strip_prefix('!') {
        let mut stmt =
            conn.prepare("SELECT name FROM waypoints WHERE name LIKE ?1 ORDER BY name LIMIT 20")?;
        let pattern = format!("{}%", wp_prefix);
        let rows = stmt.query_map([&pattern], |row| row.get::<_, String>(0))?;
        for row in rows {
            println!("!{}", row?);
        }
        return Ok(());
    }

    // Project completions for `@` prefix
    if let Some(proj_prefix) = prefix.strip_prefix('@') {
        let mut stmt =
            conn.prepare("SELECT name FROM projects WHERE name LIKE ?1 ORDER BY name LIMIT 20")?;
        let pattern = format!("{}%", proj_prefix);
        let rows = stmt.query_map([&pattern], |row| row.get::<_, String>(0))?;
        for row in rows {
            println!("@{}", row?);
        }
        return Ok(());
    }

    // Directory completions — match against last path component
    let candidates = frecency::query_all(conn, 50)?;
    let prefix_lower = prefix.to_lowercase();
    for c in candidates {
        let basename = c.path.rsplit('/').next().unwrap_or(&c.path);
        if basename.to_lowercase().starts_with(&prefix_lower) {
            println!("{}", basename);
        }
    }

    Ok(())
}

/// Entry point: parse CLI args and dispatch to the appropriate handler.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands first
    if let Some(cmd) = &cli.command {
        return match cmd {
            Commands::Init {
                shell,
                cmd,
                bootstrap,
            } => {
                if *bootstrap {
                    let conn = db::open()?;
                    crate::bootstrap::force_bootstrap(&conn)?;
                    return Ok(());
                }
                let code = shell::generate_init(shell, cmd)?;
                print!("{}", code);
                Ok(())
            }
            Commands::Import { from, path } => {
                if from != "zoxide" {
                    eprintln!(
                        "Import from '{}' is not yet supported. Supported: zoxide",
                        from
                    );
                    return Ok(());
                }

                let conn = db::open()?;

                let count = if let Some(ref file_path) = path {
                    // Read from a file provided by the user
                    let file = std::fs::File::open(file_path)
                        .with_context(|| format!("could not open file: {}", file_path))?;
                    let reader = std::io::BufReader::new(file);
                    import::import_zoxide(&conn, reader)?
                } else {
                    // Shell out to zoxide to get its data
                    let output = std::process::Command::new("zoxide")
                        .args(["query", "-l", "-s"])
                        .output()
                        .context("failed to run `zoxide query -l -s` — is zoxide installed?")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        anyhow::bail!("zoxide exited with error: {}", stderr.trim());
                    }

                    let reader = std::io::BufReader::new(std::io::Cursor::new(output.stdout));
                    import::import_zoxide(&conn, reader)?
                };

                eprintln!("Imported {} entries from zoxide.", count);
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
            Commands::Ls { count } => {
                let conn = db::open()?;
                let candidates = frecency::query_all(&conn, *count)?;
                if candidates.is_empty() {
                    eprintln!("No directories tracked yet. Navigate around to build history.");
                } else {
                    let color = crate::style::use_color();
                    let max_score = candidates.first().map(|c| c.score).unwrap_or(1.0);

                    if color {
                        // Group by project
                        let mut groups: Vec<(Option<String>, Vec<&frecency::Candidate>)> =
                            Vec::new();
                        for c in &candidates {
                            let proj = c
                                .project_root
                                .as_deref()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .map(|n| n.to_string_lossy().to_string());

                            if let Some(group) = groups.iter_mut().find(|(name, _)| *name == proj) {
                                group.1.push(c);
                            } else {
                                groups.push((proj, vec![c]));
                            }
                        }

                        for (project_name, dirs) in &groups {
                            // Project header
                            if let Some(name) = project_name {
                                let kind = dirs
                                    .first()
                                    .and_then(|c| c.project_root.as_deref())
                                    .and_then(crate::project::project_kind);
                                let icon = crate::style::project_icon(kind);
                                let branch = dirs
                                    .first()
                                    .and_then(|c| c.project_root.as_deref())
                                    .and_then(get_git_branch);
                                let branch_str = branch
                                    .map(|b| {
                                        format!(
                                            " {}({}){}",
                                            crate::style::DIM,
                                            b,
                                            crate::style::RESET
                                        )
                                    })
                                    .unwrap_or_default();

                                eprintln!(
                                    "\n  {}{}{} {}{}{}",
                                    crate::style::BOLD,
                                    crate::style::CYAN,
                                    icon,
                                    name,
                                    branch_str,
                                    crate::style::RESET
                                );
                            } else {
                                eprintln!(
                                    "\n  {}{}📁 other{}",
                                    crate::style::BOLD,
                                    crate::style::GRAY,
                                    crate::style::RESET
                                );
                            }

                            for c in dirs {
                                let sc = crate::style::score_color(c.score);
                                let bar = crate::style::score_bar(c.score, max_score);
                                let time = crate::style::relative_time(c.last_access);
                                let path = crate::style::styled_path(&c.path);

                                eprintln!(
                                    "    {}{:>6.1}{} {} {}{} {}{}{}",
                                    sc,
                                    c.score,
                                    crate::style::RESET,
                                    bar,
                                    path,
                                    crate::style::RESET,
                                    crate::style::GRAY,
                                    time,
                                    crate::style::RESET,
                                );
                            }
                        }
                        eprintln!();
                    } else {
                        for c in &candidates {
                            let project = c
                                .project_root
                                .as_deref()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .map(|n| format!(" [{}]", n.to_string_lossy()))
                                .unwrap_or_default();
                            eprintln!("{:>8.1}  {}{}", c.score, c.path, project);
                        }
                    }
                }
                Ok(())
            }
            #[cfg(feature = "tui")]
            Commands::Stats => {
                let conn = db::open()?;
                crate::tui::stats::show_stats(&conn)?;
                Ok(())
            }
            Commands::Completions { shell } => {
                let mut cmd = Cli::command();
                clap_complete::generate(*shell, &mut cmd, "tp", &mut std::io::stdout());
                Ok(())
            }
            Commands::Back { steps } => {
                let conn = db::open()?;
                match crate::nav::navigate_back(&conn, *steps)? {
                    Some(path) => {
                        println!("{}", path);
                        Ok(())
                    }
                    None => {
                        eprintln!("No navigation history to go back to.");
                        std::process::exit(1);
                    }
                }
            }
            Commands::Remove { path } => {
                let conn = db::open()?;
                let removed = frecency::remove_path(&conn, path)?;
                if removed > 0 {
                    eprintln!("Removed: {}", path);
                } else {
                    eprintln!("Not found: {}", path);
                }
                Ok(())
            }
            Commands::Query { terms, score } => {
                let conn = db::open()?;
                let joined = terms.join(" ");
                let mut candidates = frecency::query_frecency(&conn, &joined, None)?;
                // Typo-tolerant fallback when fuzzy matching finds nothing
                if candidates.is_empty() {
                    candidates = frecency::query_frecency_typo(&conn, &joined, None)?;
                }
                if candidates.is_empty() {
                    std::process::exit(1);
                }
                for c in &candidates {
                    if *score {
                        println!("{:>8.1}  {}", c.score, c.path);
                    } else {
                        println!("{}", c.path);
                    }
                }
                Ok(())
            }
            Commands::Doctor => {
                eprintln!("tp doctor");
                eprintln!("=========");
                eprintln!();

                // Database
                match db::db_path() {
                    Ok(p) => {
                        eprintln!("Database: {}", p.display());
                        if p.exists() {
                            let conn = db::open()?;
                            let dir_count: i64 =
                                conn.query_row("SELECT COUNT(*) FROM directories", [], |row| {
                                    row.get(0)
                                })?;
                            let wp_count: i64 =
                                conn.query_row("SELECT COUNT(*) FROM waypoints", [], |row| {
                                    row.get(0)
                                })?;
                            let sess_count: i64 =
                                conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| {
                                    row.get(0)
                                })?;
                            eprintln!("  Directories: {}", dir_count);
                            eprintln!("  Waypoints:   {}", wp_count);
                            eprintln!("  Sessions:    {}", sess_count);

                            // Suggestion hint
                            let suggestion_n = crate::nav::suggest::suggestion_count(&conn);
                            if suggestion_n > 0 {
                                eprintln!("  Suggestions: {} (run `tp suggest`)", suggestion_n);
                            }
                        } else {
                            eprintln!("  (not created yet — navigate once to initialize)");
                        }
                    }
                    Err(e) => eprintln!("Database: ERROR — {}", e),
                }
                eprintln!();

                // Features
                eprintln!("Features:");
                if cfg!(feature = "ai") {
                    eprintln!("  AI:  enabled");
                } else {
                    eprintln!("  AI:  disabled (rebuild with --features ai)");
                }
                if cfg!(feature = "tui") {
                    eprintln!("  TUI: enabled");
                } else {
                    eprintln!("  TUI: disabled (rebuild with --features tui)");
                }
                eprintln!();

                // AI key
                eprintln!("AI Configuration:");
                #[cfg(feature = "ai")]
                {
                    match crate::ai::detect_api_key() {
                        Some((_key, source)) => {
                            eprintln!("  API key: found in {}", source)
                        }
                        None => eprintln!("  API key: not set (run tp --setup-ai)"),
                    }
                }
                #[cfg(not(feature = "ai"))]
                {
                    eprintln!("  (AI feature not compiled)");
                }
                eprintln!();

                // Shell
                eprintln!("Environment:");
                eprintln!(
                    "  SHELL:    {}",
                    std::env::var("SHELL").unwrap_or_else(|_| "(not set)".into())
                );
                if let Ok(dir) = std::env::var("TP_DATA_DIR") {
                    eprintln!("  TP_DATA_DIR: {}", dir);
                }
                if let Ok(exclude) = std::env::var("TP_EXCLUDE_DIRS") {
                    eprintln!("  TP_EXCLUDE_DIRS: {}", exclude);
                }

                Ok(())
            }
            Commands::Index { path } => {
                let target = path.as_deref().unwrap_or(".");
                let abs = std::fs::canonicalize(target)
                    .unwrap_or_else(|_| std::path::PathBuf::from(target));
                eprintln!("tp index: semantic project indexing");
                eprintln!("  Target: {}", abs.display());

                #[cfg(feature = "ai")]
                {
                    match crate::ai::detect_api_key() {
                        Some(_) => {
                            eprintln!();
                            eprintln!("Semantic indexing is coming in a future release.");
                            eprintln!("This will let you search by concept:");
                            eprintln!("  tp the service that handles webhook retries");
                        }
                        None => {
                            eprintln!();
                            eprintln!("Requires an API key. Run: tp --setup-ai");
                        }
                    }
                }
                #[cfg(not(feature = "ai"))]
                {
                    eprintln!("AI features are not enabled. Rebuild with --features ai");
                }
                Ok(())
            }
            Commands::Analyze => {
                eprintln!("tp analyze: workflow pattern extraction");

                #[cfg(feature = "ai")]
                {
                    match crate::ai::detect_api_key() {
                        Some(_) => {
                            eprintln!();
                            eprintln!("Workflow analysis is coming in a future release.");
                            eprintln!("This will identify navigation patterns like:");
                            eprintln!("  \"After visiting auth/, you usually go to tests/auth/\"");
                        }
                        None => {
                            eprintln!();
                            eprintln!("Requires an API key. Run: tp --setup-ai");
                        }
                    }
                }
                #[cfg(not(feature = "ai"))]
                {
                    eprintln!("AI features are not enabled. Rebuild with --features ai");
                }
                Ok(())
            }
            Commands::Suggest { apply, ai, count } => {
                let conn = db::open()?;
                let mut suggestions = crate::nav::suggest::generate_suggestions(&conn, *count)?;

                if *ai {
                    crate::nav::suggest::ai_enhance_names(&mut suggestions);
                }

                if *apply {
                    crate::nav::suggest::display_suggestions(&suggestions);
                    eprintln!();
                    crate::nav::suggest::apply_suggestions(&conn, &suggestions)?;
                } else {
                    crate::nav::suggest::display_suggestions(&suggestions);
                }

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
            std::env::current_dir()?.to_string_lossy().to_string()
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
            let conn = db::open()?;
            crate::ai::recall::session_recall(&conn)?;
            return Ok(());
        }
        #[cfg(not(feature = "ai"))]
        {
            eprintln!("AI features are not enabled. Rebuild with --features ai");
            return Ok(());
        }
    }

    // Dynamic completions — output matching paths/waypoints for shell tab-complete
    if let Some(ref prefix) = cli.complete {
        let conn = db::open()?;
        return print_completions(&conn, prefix);
    }

    // Main navigation flow — bare `tp` with no args launches TUI picker,
    // but only when stdin is a real terminal. Without a TTY the TUI's
    // event::read() would block forever (e.g. running from scripts or pipes).
    let stdin_is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let interactive = cli.interactive || (cli.query.is_empty() && stdin_is_tty);

    // No query and no TTY — nothing useful to do, show help.
    if cli.query.is_empty() && !interactive {
        Cli::command().print_help()?;
        return Ok(());
    }

    let conn = db::open()?;

    // Auto-bootstrap on first use — seeds from shell history, zoxide, and project discovery
    crate::bootstrap::auto_bootstrap(&conn)?;

    match crate::nav::navigate(&conn, &cli.query, interactive, cli.project_scoped)? {
        Some(result) => {
            // Teleport effect on stderr (visual feedback)
            crate::style::teleport_effect(&result.path, &result.match_type);
            // Print path to stdout — the shell wrapper captures this and does `cd`
            println!("{}", result.path);
        }
        None => {
            let query_str = cli.query.join(" ");
            if crate::style::use_color() {
                eprint!(
                    "  {}🔍 nothing matched \"{}\"{}",
                    crate::style::YELLOW,
                    query_str,
                    crate::style::RESET
                );
                // Try to suggest a close match
                if let Ok(suggestion) = suggest_closest(&conn, &query_str) {
                    eprintln!(
                        " — {}did you mean \"{}\"?{}",
                        crate::style::DIM,
                        suggestion,
                        crate::style::RESET
                    );
                } else {
                    eprintln!();
                }
            } else {
                eprintln!("No match found for: {}", query_str);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}
