//! Terminal styling utilities using raw ANSI escape codes.
//! No dependencies — works everywhere.

use std::env;

/// Check if stderr supports color output.
/// Respects NO_COLOR (https://no-color.org/) and TP_QUIET.
pub fn use_color() -> bool {
    env::var("NO_COLOR").is_err()
        && env::var("TP_QUIET").is_err()
        && atty_stderr()
}

/// Check if stderr is a TTY (best effort, no deps).
fn atty_stderr() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::isatty(2) != 0 }
    }
    #[cfg(not(unix))]
    {
        true // Assume color on non-unix, NO_COLOR will override
    }
}

// ── ANSI codes ──────────────────────────────────────────────────

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const ITALIC: &str = "\x1b[3m";

// Foreground colors
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const CYAN: &str = "\x1b[36m";
pub const WHITE: &str = "\x1b[37m";
pub const GRAY: &str = "\x1b[90m";

// Bright foreground
pub const BRIGHT_YELLOW: &str = "\x1b[93m";
pub const BRIGHT_CYAN: &str = "\x1b[96m";

/// Project type to emoji icon mapping.
pub fn project_icon(kind: Option<&str>) -> &'static str {
    match kind {
        Some("rust") => "🦀",
        Some("node") => "📦",
        Some("python") => "🐍",
        Some("go") => "🐹",
        Some("ruby") => "💎",
        Some("java") => "☕",
        Some("php") => "🐘",
        Some("elixir") => "💧",
        Some("cmake") | Some("make") => "🔧",
        Some("nix") => "❄️",
        Some("deno") => "🦕",
        Some("git") => "📂",
        _ => "📁",
    }
}

/// Score to color — hotter scores get warmer colors.
pub fn score_color(score: f64) -> &'static str {
    if score >= 20.0 {
        RED
    } else if score >= 8.0 {
        YELLOW
    } else if score >= 2.0 {
        GREEN
    } else {
        GRAY
    }
}

/// Score to bar visualization (max 8 chars).
pub fn score_bar(score: f64, max_score: f64) -> String {
    let ratio = if max_score > 0.0 {
        (score / max_score).min(1.0)
    } else {
        0.0
    };
    let filled = (ratio * 8.0).round() as usize;
    let empty = 8 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Format seconds since epoch as relative time string.
pub fn relative_time(last_access: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let elapsed = (now - last_access).max(0);

    match elapsed {
        s if s < 60 => "just now".to_string(),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86400 => format!("{}h ago", s / 3600),
        s if s < 604800 => format!("{}d ago", s / 86400),
        s if s < 2592000 => format!("{}w ago", s / 604800),
        _ => format!("{}mo ago", elapsed / 2592000),
    }
}

/// Dim the home directory prefix in a path, brighten the unique part.
pub fn styled_path(path: &str) -> String {
    if let Ok(home) = env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("{}~{}{}{}", DIM, RESET, BOLD, rest);
        }
    }
    format!("{}{}", BOLD, path)
}

/// Print the teleport effect to stderr with randomized flavor text.
pub fn teleport_effect(path: &str, match_type: &str) {
    if !use_color() {
        return;
    }

    if match_type == "literal" {
        return; // No effect for plain cd
    }

    let display_path = if let Ok(home) = env::var("HOME") {
        path.replace(&home, "~")
    } else {
        path.to_string()
    };

    // Pick a random flavor based on match type
    let flavor = match match_type {
        "waypoint" => pick_random(&["📌 pinned →", "📌 waypoint →"]),
        "project" => pick_random(&["📂 project →", "📂 entering →", "📂 switching to →"]),
        "picker" => pick_random(&["🎯 selected →", "🎯 targeted →"]),
        "frecency" => pick_random(&[
            "⚡ teleported →",
            "⚡ warped →",
            "⚡ blinked →",
            "⚡ fast traveled →",
        ]),
        "ai" => pick_random(&["🔮 divined →", "🔮 the oracle says →", "🔮 foretold →"]),
        "typo" => pick_random(&["🔧 close enough →", "🔧 you meant →", "🔧 autocorrected →"]),
        _ => pick_random(&["⚡ →", "⚡ teleported →", "⚡ warped →"]),
    };

    eprintln!("{}{} {}{}", CYAN, flavor, display_path, RESET);
}

/// BluePulse spinner frames and interval.
pub const SPINNER_FRAMES: &[&str] = &["🔹", "🔷", "🔵", "🔵", "🔷"];
pub const SPINNER_INTERVAL_MS: u64 = 100;

/// A simple terminal spinner that runs on a background thread.
pub struct Spinner {
    handle: Option<std::thread::JoinHandle<()>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Spinner {
    /// Start the spinner with a message. Prints to stderr.
    pub fn start(message: &str) -> Self {
        if !use_color() {
            return Spinner {
                handle: None,
                stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            };
        }

        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_clone = stop.clone();
        let msg = message.to_string();

        let handle = std::thread::spawn(move || {
            let mut i = 0;
            // Hide cursor
            eprint!("\x1b[?25l");
            while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let frame = SPINNER_FRAMES[i % SPINNER_FRAMES.len()];
                eprint!("\r  {} {}{}{}", frame, CYAN, msg, RESET);
                i += 1;
                std::thread::sleep(std::time::Duration::from_millis(SPINNER_INTERVAL_MS));
            }
            // Clear the line and show cursor
            eprint!("\r\x1b[2K\x1b[?25h");
        });

        Spinner {
            handle: Some(handle),
            stop,
        }
    }

    /// Stop the spinner and clean up.
    pub fn stop(self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle {
            let _ = handle.join();
        }
    }
}

/// Pick a random element from a slice using a simple time-based seed.
fn pick_random<'a>(options: &[&'a str]) -> &'a str {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    options[seed % options.len()]
}

/// Print the first-run welcome message.
pub fn welcome_message(dirs: usize, zoxide: usize, projects: usize) {
    if !use_color() {
        eprintln!("tp: imported {} dirs, {} from zoxide, {} projects discovered", dirs, zoxide, projects);
        return;
    }

    eprintln!();
    eprintln!("  {}{}◈ tp — fast travel unlocked{}", BOLD, CYAN, RESET);
    eprintln!();
    if dirs > 0 {
        eprintln!("  {}Found {} directories in shell history{}", DIM, dirs, RESET);
    }
    if zoxide > 0 {
        eprintln!("  {}Imported {} entries from zoxide{}", DIM, zoxide, RESET);
    }
    if projects > 0 {
        eprintln!("  {}Discovered {} projects{}", DIM, projects, RESET);
    }
    eprintln!();
    eprintln!("  {}You're ready. Try: {}tp <project-name>{}", DIM, RESET, RESET);
    eprintln!();
}
