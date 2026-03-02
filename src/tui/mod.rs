pub mod stats;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

use crate::nav::frecency::Candidate;
use crate::nav::matching;

/// Display-friendly representation of a candidate directory.
struct CandidateDisplay {
    path: String,
    score: f64,
    project_name: Option<String>,
    project_kind: Option<String>,
    git_branch: Option<String>,
    relative_time: String,
}

/// Application state for the TUI picker.
struct App {
    input: String,
    all_candidates: Vec<CandidateDisplay>,
    filtered: Vec<usize>, // indices into all_candidates
    list_state: ListState,
}

impl App {
    /// Build an App from raw Candidate data. Converts each Candidate into a
    /// CandidateDisplay (extracting project name, git branch, relative time),
    /// then initialises the filtered list to show everything.
    fn new(candidates: &[Candidate]) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Cache git branches per project root to avoid spawning hundreds of
        // git processes. Most candidates share the same project root, so this
        // reduces ~200 subprocess spawns down to a handful.
        let mut branch_cache: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::new();

        let all_candidates: Vec<CandidateDisplay> = candidates
            .iter()
            .map(|c| {
                let project_name = c
                    .project_root
                    .as_deref()
                    .and_then(|pr| pr.rsplit('/').next())
                    .map(|s| s.to_string());

                // Look up git branch from the project root (cached).
                // Only fall back to per-path lookup if there's no project root.
                let git_branch = if let Some(root) = c.project_root.as_deref() {
                    branch_cache
                        .entry(root.to_string())
                        .or_insert_with(|| get_git_branch(root))
                        .clone()
                } else {
                    None
                };

                let project_kind = c
                    .project_root
                    .as_deref()
                    .and_then(crate::project::project_kind)
                    .map(|s| s.to_string());

                let relative_time = format_relative_time(c.last_access, now);

                CandidateDisplay {
                    path: c.path.clone(),
                    score: c.score,
                    project_name,
                    project_kind,
                    git_branch,
                    relative_time,
                }
            })
            .collect();

        let filtered: Vec<usize> = (0..all_candidates.len()).collect();
        let mut list_state = ListState::default();
        if !filtered.is_empty() {
            list_state.select(Some(0));
        }

        App {
            input: String::new(),
            all_candidates,
            filtered,
            list_state,
        }
    }

    /// Re-filter candidates against the current input. If the input is empty
    /// every candidate passes; otherwise we use fuzzy_score > 0.0.
    fn apply_filter(&mut self) {
        if self.input.is_empty() {
            self.filtered = (0..self.all_candidates.len()).collect();
        } else {
            self.filtered = self
                .all_candidates
                .iter()
                .enumerate()
                .filter(|(_, c)| matching::fuzzy_score(&self.input, &c.path) > 0.0)
                .map(|(i, _)| i)
                .collect();
        }
        // Reset selection to the top of the new filtered list.
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn move_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected > 0 {
                self.list_state.select(Some(selected - 1));
            }
        }
    }

    fn move_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected + 1 < self.filtered.len() {
                self.list_state.select(Some(selected + 1));
            }
        }
    }

    /// Return the path of the currently highlighted candidate, if any.
    fn selected_path(&self) -> Option<String> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered.get(i))
            .map(|&idx| self.all_candidates[idx].path.clone())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Ask git for the current branch name in `project_root`.
/// Returns None on any error or if HEAD is detached.
fn get_git_branch(project_root: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", project_root, "rev-parse", "--abbrev-ref", "HEAD"])
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

/// Turn an epoch timestamp into a human-friendly relative string.
fn format_relative_time(epoch: i64, now: i64) -> String {
    let diff = (now - epoch).max(0);
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Show an interactive TUI picker and return the selected path (or None if
/// the user cancelled / there were no candidates).
pub fn pick(candidates: &[Candidate]) -> Result<Option<String>> {
    if candidates.is_empty() {
        return Ok(None);
    }

    // Render the TUI to stderr so it stays visible even when the shell
    // wrapper captures stdout (e.g. `result="$(command tp)"`).  stdout
    // is reserved for the final path that the wrapper `cd`s into.
    enable_raw_mode()?;
    std::io::stderr().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(std::io::stderr());
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(candidates);
    let result = run_event_loop(&mut terminal, app);

    // Always clean up, even if the loop errored.
    disable_raw_mode()?;
    std::io::stderr().execute(LeaveAlternateScreen)?;

    result
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stderr>>,
    mut app: App,
) -> Result<Option<String>> {
    loop {
        terminal.draw(|f| render(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            // Only react to key-press, not release or repeat.
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Enter => return Ok(app.selected_path()),
                KeyCode::Up => app.move_up(),
                KeyCode::Down => app.move_down(),
                KeyCode::Backspace => {
                    app.input.pop();
                    app.apply_filter();
                }
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.move_down();
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.move_up();
                }
                KeyCode::Char(c) => {
                    app.input.push(c);
                    app.apply_filter();
                }
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // input area
            Constraint::Min(1),    // candidate list
            Constraint::Length(1), // status bar
        ])
        .split(f.area());

    // --- Input area ---
    let input_color = if app.filtered.is_empty() && !app.input.is_empty() {
        Color::Red
    } else if !app.input.is_empty() {
        Color::Green
    } else {
        Color::White
    };
    let input_text = if app.input.is_empty() {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::styled("type to filter...", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::styled(&app.input, Style::default().fg(input_color)),
        ])
    };
    let input_paragraph = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" tp — where to? "),
    );
    f.render_widget(input_paragraph, chunks[0]);

    // --- Candidate list ---
    let max_score = app
        .filtered
        .first()
        .map(|&idx| app.all_candidates[idx].score)
        .unwrap_or(1.0)
        .max(0.001);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let c = &app.all_candidates[idx];
            let is_selected = app.list_state.selected() == Some(i);

            // Fade candidates that score less than 20% of the top result
            let is_faded = !is_selected && (c.score / max_score) < 0.2;

            let prefix = if is_selected { "→ " } else { "  " };
            let path_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if is_faded {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };

            // Dim home prefix, highlight the unique part
            let home = std::env::var("HOME").unwrap_or_default();
            let path_spans = if !home.is_empty() && c.path.starts_with(&home) {
                let rest = &c.path[home.len()..];
                vec![
                    Span::raw(prefix),
                    Span::styled("~", Style::default().fg(Color::Gray)),
                    Span::styled(rest.to_string(), path_style),
                ]
            } else {
                vec![Span::raw(prefix), Span::styled(c.path.clone(), path_style)]
            };

            let line1 = Line::from(path_spans);

            // Build metadata pieces: icon project name · git branch · relative time
            let mut meta_parts: Vec<String> = Vec::new();
            if let Some(ref name) = c.project_name {
                let icon = crate::style::project_icon(c.project_kind.as_deref());
                meta_parts.push(format!("{} {}", icon, name));
            }
            if let Some(ref branch) = c.git_branch {
                meta_parts.push(branch.clone());
            }
            meta_parts.push(c.relative_time.clone());

            let meta_color = if is_selected {
                Color::Rgb(140, 140, 140) // bright enough to read when highlighted
            } else if is_faded {
                Color::DarkGray
            } else {
                Color::Gray // visible but secondary
            };
            let line2 = Line::from(Span::styled(
                format!("    {}", meta_parts.join(" · ")),
                Style::default().fg(meta_color),
            ));

            ListItem::new(vec![line1, line2])
        })
        .collect();

    let list = List::new(items);
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // --- Status bar ---
    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {}/{} ", app.filtered.len(), app.all_candidates.len()),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            "↑↓",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "⏎",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" select  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "esc",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(status, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::frecency::Candidate;

    /// Helper to build a simple candidate for tests.
    fn make_candidate(path: &str, last_access: i64) -> Candidate {
        Candidate {
            path: path.to_string(),
            score: 1.0,
            frecency: 1.0,
            last_access,
            access_count: 1,
            project_root: None,
        }
    }

    #[test]
    fn test_pick_empty() {
        // Early-return path: no TUI is started.
        let result = pick(&[]).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_format_relative_time() {
        let now = 100_000;
        assert_eq!(format_relative_time(now, now), "just now");
        assert_eq!(format_relative_time(now - 120, now), "2m ago");
        assert_eq!(format_relative_time(now - 7200, now), "2h ago");
        assert_eq!(format_relative_time(now - 172800, now), "2d ago");
    }

    #[test]
    fn test_app_filtering() {
        let candidates = vec![
            make_candidate("/home/user/projects/api", 0),
            make_candidate("/home/user/projects/web", 0),
        ];
        let mut app = App::new(&candidates);

        // Initially all candidates are visible.
        assert_eq!(app.filtered.len(), 2);

        // Filter to "api" — only one candidate matches (fuzzy_score > 0).
        app.input = "api".to_string();
        app.apply_filter();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(
            app.selected_path(),
            Some("/home/user/projects/api".to_string())
        );
    }

    #[test]
    fn test_app_navigation() {
        let candidates = vec![make_candidate("/first", 0), make_candidate("/second", 0)];
        let mut app = App::new(&candidates);

        // Starts at index 0.
        assert_eq!(app.list_state.selected(), Some(0));

        // Move down to index 1.
        app.move_down();
        assert_eq!(app.list_state.selected(), Some(1));

        // Move down again — should clamp at 1 (last item).
        app.move_down();
        assert_eq!(app.list_state.selected(), Some(1));

        // Move back up.
        app.move_up();
        assert_eq!(app.list_state.selected(), Some(0));

        // Move up again — should clamp at 0.
        app.move_up();
        assert_eq!(app.list_state.selected(), Some(0));
    }
}
