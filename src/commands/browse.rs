//! `arf browse` - interactive ratatui TUI over commits + ARF records.
//!
//! Three panes: a commits list, a reasoning panel for the selected
//! commit, and an optional diff panel (toggleable Stat / Full /
//! Hidden via `d`). j/k navigate, Tab swaps focus, q quits.

use crate::record::ArfRecord;
use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::io::stdout;
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
struct CommitInfo {
    sha: String,
    short_sha: String,
    message: String,
    records: Vec<ArfRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DiffMode {
    Hidden,
    Stat,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    Commits,
    Diff,
}

struct App {
    commits: Vec<CommitInfo>,
    list_state: ListState,
    diff_mode: DiffMode,
    diff_lines: Vec<DiffLine>,
    diff_scroll: usize,
    focus: Focus,
    should_quit: bool,
}

#[derive(Debug, Clone)]
struct DiffLine {
    content: String,
    style: Style,
}

impl App {
    fn new(commits: Vec<CommitInfo>) -> Self {
        let mut list_state = ListState::default();
        if !commits.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            commits,
            list_state,
            diff_mode: DiffMode::Stat,
            diff_lines: Vec::new(),
            diff_scroll: 0,
            focus: Focus::Commits,
            should_quit: false,
        }
    }

    fn selected_commit(&self) -> Option<&CommitInfo> {
        self.list_state.selected().and_then(|i| self.commits.get(i))
    }

    fn next(&mut self) {
        match self.focus {
            Focus::Commits => {
                if self.commits.is_empty() {
                    return;
                }
                let i = match self.list_state.selected() {
                    Some(i) => (i + 1) % self.commits.len(),
                    None => 0,
                };
                self.list_state.select(Some(i));
                self.diff_scroll = 0;
                self.update_diff();
            }
            Focus::Diff => {
                if self.diff_scroll < self.diff_lines.len().saturating_sub(1) {
                    self.diff_scroll += 1;
                }
            }
        }
    }

    fn previous(&mut self) {
        match self.focus {
            Focus::Commits => {
                if self.commits.is_empty() {
                    return;
                }
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.commits.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
                self.diff_scroll = 0;
                self.update_diff();
            }
            Focus::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
        }
    }

    fn toggle_focus(&mut self) {
        if self.diff_mode != DiffMode::Hidden {
            self.focus = match self.focus {
                Focus::Commits => Focus::Diff,
                Focus::Diff => Focus::Commits,
            };
        }
    }

    fn toggle_diff(&mut self) {
        self.diff_mode = match self.diff_mode {
            DiffMode::Hidden => DiffMode::Stat,
            DiffMode::Stat => DiffMode::Full,
            DiffMode::Full => DiffMode::Hidden,
        };
        if self.diff_mode == DiffMode::Hidden {
            self.focus = Focus::Commits;
        }
        self.diff_scroll = 0;
        self.update_diff();
    }

    fn page_down(&mut self) {
        if self.focus == Focus::Diff {
            self.diff_scroll = (self.diff_scroll + 10).min(self.diff_lines.len().saturating_sub(1));
        }
    }

    fn page_up(&mut self) {
        if self.focus == Focus::Diff {
            self.diff_scroll = self.diff_scroll.saturating_sub(10);
        }
    }

    fn update_diff(&mut self) {
        self.diff_lines.clear();

        if self.diff_mode == DiffMode::Hidden {
            return;
        }

        let Some(commit) = self.selected_commit() else {
            return;
        };

        let args = if self.diff_mode == DiffMode::Full {
            vec!["show", "--format=", &commit.sha]
        } else {
            vec!["show", "--stat", "--format=", &commit.sha]
        };

        let output = Command::new("git").args(&args).output();
        let content = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => "Failed to get diff".to_string(),
        };

        for line in content.lines() {
            let (style, display) = if line.starts_with('+') && !line.starts_with("+++") {
                (Style::default().fg(Color::Green), line.to_string())
            } else if line.starts_with('-') && !line.starts_with("---") {
                (Style::default().fg(Color::Red), line.to_string())
            } else if line.starts_with("@@") {
                (Style::default().fg(Color::Cyan), line.to_string())
            } else if line.starts_with("diff ") || line.starts_with("index ") {
                (Style::default().fg(Color::Yellow).bold(), line.to_string())
            } else if line.starts_with("+++") || line.starts_with("---") {
                (Style::default().fg(Color::Yellow), line.to_string())
            } else {
                (Style::default(), line.to_string())
            };

            self.diff_lines.push(DiffLine {
                content: display,
                style,
            });
        }
    }
}

pub fn run() -> Result<()> {
    let output = Command::new("git")
        .args(["log", "--oneline", "--no-decorate", "-50"])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("Failed to get git log"));
    }

    let log = String::from_utf8_lossy(&output.stdout);
    let records_dir = Path::new(".arf/records");

    let mut commits: Vec<CommitInfo> = Vec::new();

    for line in log.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let (sha, msg) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (parts[0], "")
        };

        let short_sha = sha.to_string();

        let full_sha = Command::new("git")
            .args(["rev-parse", sha])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| sha.to_string());

        let mut records = Vec::new();
        if records_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(records_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    if dir_name.starts_with(&short_sha) || short_sha.starts_with(&dir_name) {
                        if let Ok(record_entries) = std::fs::read_dir(entry.path()) {
                            for record_entry in record_entries.filter_map(|e| e.ok()) {
                                let path = record_entry.path();
                                if path.extension().is_some_and(|e| e == "toml") {
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        if let Ok(record) = toml::from_str::<ArfRecord>(&content) {
                                            records.push(record);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        commits.push(CommitInfo {
            sha: full_sha,
            short_sha,
            message: msg.to_string(),
            records,
        });
    }

    if commits.is_empty() {
        println!("No commits found.");
        return Ok(());
    }

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new(commits);
    app.update_diff();

    loop {
        terminal.draw(|frame| ui(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char('d') => app.toggle_diff(),
                    KeyCode::Tab | KeyCode::Enter => app.toggle_focus(),
                    KeyCode::PageDown | KeyCode::Char('f') => app.page_down(),
                    KeyCode::PageUp | KeyCode::Char('b') => app.page_up(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    let has_diff = app.diff_mode != DiffMode::Hidden;

    let focused_border = Style::default().fg(Color::Cyan);
    let unfocused_border = Style::default();

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_diff {
            vec![Constraint::Percentage(50), Constraint::Percentage(50)]
        } else {
            vec![Constraint::Percentage(100)]
        })
        .split(frame.area());

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[0]);

    let items: Vec<ListItem> = app
        .commits
        .iter()
        .map(|c| {
            let has_arf = if c.records.is_empty() { " " } else { "●" };
            ListItem::new(format!("{} {} {}", has_arf, c.short_sha, c.message))
        })
        .collect();

    let commits_border = if app.focus == Focus::Commits {
        focused_border
    } else {
        unfocused_border
    };

    let commits_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(commits_border)
                .title(" Commits "),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).bold())
        .highlight_symbol("→ ");

    frame.render_stateful_widget(commits_list, top_chunks[0], &mut app.list_state);

    let reasoning_text = if let Some(commit) = app.selected_commit() {
        if commit.records.is_empty() {
            "(no ARF record for this commit)".to_string()
        } else {
            commit
                .records
                .iter()
                .map(|r| {
                    let mut s = format!("what: {}\nwhy:  {}", r.what, r.why);
                    if let Some(ref how) = r.how {
                        s.push_str(&format!("\nhow:  {}", how));
                    }
                    if let Some(ref backup) = r.backup {
                        s.push_str(&format!("\nback: {}", backup));
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n")
        }
    } else {
        "No commit selected".to_string()
    };

    let reasoning = Paragraph::new(reasoning_text)
        .block(Block::default().borders(Borders::ALL).title(" Reasoning "))
        .wrap(Wrap { trim: false });

    frame.render_widget(reasoning, top_chunks[1]);

    if has_diff {
        let diff_border = if app.focus == Focus::Diff {
            focused_border
        } else {
            unfocused_border
        };

        let diff_title = match app.diff_mode {
            DiffMode::Stat => " Diff (stat) ",
            DiffMode::Full => " Diff (full) ",
            DiffMode::Hidden => "",
        };

        let lines: Vec<Line> = app
            .diff_lines
            .iter()
            .skip(app.diff_scroll)
            .map(|dl| Line::from(Span::styled(dl.content.clone(), dl.style)))
            .collect();

        let scroll_info = if !app.diff_lines.is_empty() {
            format!(" [{}/{}] ", app.diff_scroll + 1, app.diff_lines.len())
        } else {
            String::new()
        };

        let diff = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(diff_border)
                .title(format!("{}{}", diff_title, scroll_info)),
        );

        frame.render_widget(diff, main_chunks[1]);
    }

    let help = " q: quit | j/k: scroll | Tab: focus | d: toggle diff | f/b: page ";
    let help_area = Rect {
        x: 0,
        y: frame.area().height - 1,
        width: frame.area().width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::default().bg(Color::DarkGray)),
        help_area,
    );
}
