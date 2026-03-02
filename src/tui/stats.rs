use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use rusqlite::Connection;

/// Navigation statistics gathered from the database.
struct Stats {
    total_dirs: usize,
    total_visits: i64,
    total_waypoints: usize,
    total_projects: usize,
    top_dirs: Vec<(String, f64)>,            // (basename, score)
    project_breakdown: Vec<(String, usize)>, // (project_name, dir_count)
    hourly_heatmap: [i64; 24],               // visits per hour of day
    visits_today: i64,
    waypoints: Vec<(String, String)>, // (name, path)
}

/// Animation state for the dashboard.
struct AnimState {
    progress: f64, // 0.0 to 1.0
    frame: usize,
}

fn gather_stats(conn: &Connection) -> Result<Stats> {
    let total_dirs: usize =
        conn.query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))?;

    let total_visits: i64 = conn.query_row(
        "SELECT COALESCE(SUM(access_count), 0) FROM directories",
        [],
        |row| row.get(0),
    )?;

    let total_waypoints: usize =
        conn.query_row("SELECT COUNT(*) FROM waypoints", [], |row| row.get(0))?;

    let total_projects: usize = conn.query_row(
        "SELECT COUNT(DISTINCT project_root) FROM directories WHERE project_root IS NOT NULL",
        [],
        |row| row.get(0),
    )?;

    // Top directories by frecency
    let mut stmt =
        conn.prepare("SELECT path, frecency FROM directories ORDER BY frecency DESC LIMIT 10")?;
    let top_dirs: Vec<(String, f64)> = stmt
        .query_map([], |row| {
            let path: String = row.get(0)?;
            let score: f64 = row.get(1)?;
            let basename = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
            Ok((basename, score))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Project breakdown
    let mut stmt = conn.prepare(
        "SELECT project_root, COUNT(*) as cnt FROM directories
         WHERE project_root IS NOT NULL
         GROUP BY project_root ORDER BY cnt DESC LIMIT 8",
    )?;
    let project_breakdown: Vec<(String, usize)> = stmt
        .query_map([], |row| {
            let root: String = row.get(0)?;
            let count: usize = row.get(1)?;
            let name = root.rsplit(['/', '\\']).next().unwrap_or(&root).to_string();
            Ok((name, count))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Hourly heatmap from sessions
    let mut hourly = [0i64; 24];
    let mut stmt = conn.prepare("SELECT timestamp FROM sessions WHERE timestamp IS NOT NULL")?;
    let timestamps: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for ts in &timestamps {
        // Convert unix timestamp to hour of day (UTC)
        let hour = ((*ts % 86400) / 3600) as usize;
        if hour < 24 {
            hourly[hour] += 1;
        }
    }

    // Visits today
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let today_start = now - (now % 86400);
    let visits_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE timestamp >= ?1",
        [today_start],
        |row| row.get(0),
    )?;

    // Waypoints
    let mut stmt = conn.prepare("SELECT name, path FROM waypoints ORDER BY name LIMIT 20")?;
    let waypoints: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Stats {
        total_dirs,
        total_visits,
        total_waypoints,
        total_projects,
        top_dirs,
        project_breakdown,
        hourly_heatmap: hourly,
        visits_today,
        waypoints,
    })
}

pub fn show_stats(conn: &Connection) -> Result<()> {
    let mut stats = gather_stats(conn)?;

    let mut stdout = std::io::stderr();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut anim = AnimState {
        progress: 0.0,
        frame: 0,
    };

    // Animation loop: ~20 frames over ~400ms
    let anim_frames = 20;

    loop {
        if anim.frame <= anim_frames {
            anim.progress = (anim.frame as f64 / anim_frames as f64).min(1.0);
            // Ease-out curve
            anim.progress = 1.0 - (1.0 - anim.progress).powi(3);
            anim.frame += 1;
        }

        terminal.draw(|frame| {
            let size = frame.area();

            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(1),
                ])
                .split(size);

            // Header
            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    "◈ ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "tp stats",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    format!(
                        "{} dirs · {} visits · {} waypoints · {} projects",
                        stats.total_dirs,
                        stats.total_visits,
                        stats.total_waypoints,
                        stats.total_projects
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
            frame.render_widget(header, main_chunks[0]);

            // Body: two columns
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(main_chunks[1]);

            // Left: top dirs + heatmap
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(8), Constraint::Length(6)])
                .split(body_chunks[0]);

            render_top_dirs(frame, left_chunks[0], &stats, anim.progress);
            render_heatmap(frame, left_chunks[1], &stats);

            // Right: projects + waypoints + today
            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(4),
                    Constraint::Length(if stats.waypoints.is_empty() {
                        0
                    } else {
                        (stats.waypoints.len() as u16 + 2).min(8)
                    }),
                    Constraint::Length(5),
                ])
                .split(body_chunks[1]);

            render_projects(frame, right_chunks[0], &stats);
            if !stats.waypoints.is_empty() {
                render_waypoints(frame, right_chunks[1], &stats);
            }
            render_today(frame, right_chunks[2], &stats);

            // Footer
            let footer = Paragraph::new(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "r",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "q",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" quit", Style::default().fg(Color::DarkGray)),
            ]));
            frame.render_widget(footer, main_chunks[2]);
        })?;

        // During animation, use short timeout for smooth frames
        if anim.frame <= anim_frames {
            if crossterm::event::poll(std::time::Duration::from_millis(20))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('r') => {
                                stats = gather_stats(conn)?;
                                anim.frame = 0;
                                anim.progress = 0.0;
                            }
                            _ => {}
                        }
                    }
                }
            }
        } else {
            // Static: block until keypress
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('r') => {
                            stats = gather_stats(conn)?;
                            anim.frame = 0;
                            anim.progress = 0.0;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn render_top_dirs(frame: &mut ratatui::Frame, area: Rect, stats: &Stats, progress: f64) {
    let max_score = stats.top_dirs.first().map(|(_, s)| *s).unwrap_or(1.0);

    let items: Vec<Line> = stats
        .top_dirs
        .iter()
        .enumerate()
        .map(|(i, (name, score))| {
            // Animate bar width based on progress
            let target_width = ((score / max_score) * 20.0).round() as usize;
            let bar_width = ((target_width as f64) * progress).round() as usize;
            let bar = "█".repeat(bar_width);
            let pad = " ".repeat(20 - bar_width);

            let color = if i == 0 {
                Color::Red
            } else if i < 3 {
                Color::Yellow
            } else if i < 6 {
                Color::Green
            } else {
                Color::DarkGray
            };

            Line::from(vec![
                Span::styled(format!("{:>6.0} ", score), Style::default().fg(color)),
                Span::styled(bar, Style::default().fg(color)),
                Span::raw(pad),
                Span::styled(format!(" {}", name), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            " top directories ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(items).block(block);
    frame.render_widget(paragraph, area);
}

fn render_heatmap(frame: &mut ratatui::Frame, area: Rect, stats: &Stats) {
    let max_val = *stats.hourly_heatmap.iter().max().unwrap_or(&1);

    let hours: Vec<Line> = vec![
        // Row 1: bar visualization
        Line::from(
            stats
                .hourly_heatmap
                .iter()
                .map(|&count| {
                    let intensity = if max_val > 0 {
                        (count as f64 / max_val as f64 * 4.0).round() as usize
                    } else {
                        0
                    };
                    let ch = match intensity {
                        0 => "·",
                        1 => "░",
                        2 => "▒",
                        3 => "▓",
                        _ => "█",
                    };
                    let color = match intensity {
                        0 => Color::DarkGray,
                        1 => Color::Blue,
                        2 => Color::Cyan,
                        3 => Color::Yellow,
                        _ => Color::Red,
                    };
                    Span::styled(format!("{} ", ch), Style::default().fg(color))
                })
                .collect::<Vec<_>>(),
        ),
        // Row 2: hour labels
        Line::from(
            (0..24)
                .map(|h| Span::styled(format!("{:>2}", h), Style::default().fg(Color::DarkGray)))
                .collect::<Vec<_>>(),
        ),
    ];

    let block = Block::default()
        .title(Span::styled(
            " activity (hour of day, UTC) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(hours).block(block);
    frame.render_widget(paragraph, area);
}

fn render_projects(frame: &mut ratatui::Frame, area: Rect, stats: &Stats) {
    let max_count = stats
        .project_breakdown
        .first()
        .map(|(_, c)| *c)
        .unwrap_or(1);

    let items: Vec<Line> = stats
        .project_breakdown
        .iter()
        .map(|(name, count)| {
            let bar_width = ((*count as f64 / max_count as f64) * 15.0).round() as usize;
            let bar = "█".repeat(bar_width);

            Line::from(vec![
                Span::styled(format!("{:>4} ", count), Style::default().fg(Color::Yellow)),
                Span::styled(bar, Style::default().fg(Color::Magenta)),
                Span::styled(format!(" {}", name), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            " projects ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(items).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_waypoints(frame: &mut ratatui::Frame, area: Rect, stats: &Stats) {
    let items: Vec<Line> = stats
        .waypoints
        .iter()
        .map(|(name, path)| {
            let display_path = std::env::var("HOME")
                .ok()
                .and_then(|home| path.strip_prefix(&home).map(|rest| format!("~{}", rest)))
                .unwrap_or_else(|| path.clone());

            Line::from(vec![
                Span::styled(
                    format!("  !{:<12}", name),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(display_path, Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            " waypoints ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(items).block(block);
    frame.render_widget(paragraph, area);
}

fn render_today(frame: &mut ratatui::Frame, area: Rect, stats: &Stats) {
    let lines = vec![
        Line::from(vec![
            Span::styled("  Teleports today: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", stats.visits_today),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Total lifetime:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", stats.total_visits),
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let block = Block::default()
        .title(Span::styled(
            " today ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
