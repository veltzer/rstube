use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::io::{Stdout, stdout};
use std::time::Duration;

use crate::state::{self, HistoryEntry};

pub struct Selection {
    pub url: String,
    pub title: Option<String>,
    pub duration_secs: Option<f64>,
    pub audio_only: bool,
}

enum Focus {
    List,
    Filter,
}

pub fn run_picker() -> Result<Option<Selection>> {
    let mut entries = state::load_history_deduped();
    if entries.is_empty() {
        return Ok(None);
    }

    let mut terminal = setup_terminal().context("failed to init terminal")?;
    let result = main_loop(&mut terminal, &mut entries);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn main_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    entries: &mut Vec<HistoryEntry>,
) -> Result<Option<Selection>> {
    let mut filter = String::new();
    let mut focus = Focus::List;
    let mut list_state = ListState::default();
    list_state.select(Some(0));
    let mut audio_only = false;

    loop {
        let filtered = filter_entries(entries, &filter);
        if list_state.selected().unwrap_or(0) >= filtered.len() && !filtered.is_empty() {
            list_state.select(Some(filtered.len() - 1));
        }
        if filtered.is_empty() {
            list_state.select(None);
        } else if list_state.selected().is_none() {
            list_state.select(Some(0));
        }

        terminal.draw(|f| draw(f, &filtered, &mut list_state, &filter, &focus, audio_only))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(None);
        }

        match focus {
            Focus::Filter => match key.code {
                KeyCode::Esc | KeyCode::Enter => focus = Focus::List,
                KeyCode::Backspace => { filter.pop(); }
                KeyCode::Char(c) => filter.push(c),
                _ => {}
            },
            Focus::List => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                KeyCode::Char('/') => focus = Focus::Filter,
                KeyCode::Char('a') => audio_only = !audio_only,
                KeyCode::Down | KeyCode::Char('j') => move_selection(&mut list_state, &filtered, 1),
                KeyCode::Up | KeyCode::Char('k') => move_selection(&mut list_state, &filtered, -1),
                KeyCode::PageDown => move_selection(&mut list_state, &filtered, 10),
                KeyCode::PageUp => move_selection(&mut list_state, &filtered, -10),
                KeyCode::Home => {
                    if !filtered.is_empty() { list_state.select(Some(0)); }
                }
                KeyCode::End => {
                    if !filtered.is_empty() { list_state.select(Some(filtered.len() - 1)); }
                }
                KeyCode::Char('d') => {
                    if let Some(idx) = list_state.selected()
                        && let Some(entry) = filtered.get(idx)
                    {
                        let id = entry.video_id.clone();
                        entries.retain(|e| e.video_id != id);
                        let _ = remove_position(&id);
                    }
                }
                KeyCode::Enter => {
                    if let Some(idx) = list_state.selected()
                        && let Some(entry) = filtered.get(idx)
                    {
                        return Ok(Some(Selection {
                            url: entry.url.clone(),
                            title: entry.title.clone(),
                            duration_secs: entry.duration_secs,
                            audio_only,
                        }));
                    }
                }
                _ => {}
            },
        }
    }
}

fn move_selection(state: &mut ListState, items: &[&HistoryEntry], delta: i32) {
    if items.is_empty() {
        return;
    }
    let len = items.len() as i32;
    let current = state.selected().unwrap_or(0) as i32;
    let next = (current + delta).clamp(0, len - 1);
    state.select(Some(next as usize));
}

fn filter_entries<'a>(entries: &'a [HistoryEntry], filter: &str) -> Vec<&'a HistoryEntry> {
    if filter.is_empty() {
        return entries.iter().collect();
    }
    let needle = filter.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            let hay = e.title.as_deref().unwrap_or(&e.url).to_lowercase();
            hay.contains(&needle)
        })
        .collect()
}

fn remove_position(video_id: &str) -> Result<()> {
    let mut positions = state::load_positions();
    positions.remove(video_id);
    state::save_positions(&positions)
}

fn draw(
    f: &mut ratatui::Frame,
    items: &[&HistoryEntry],
    list_state: &mut ListState,
    filter: &str,
    focus: &Focus,
    audio_only: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    let filter_title = match focus {
        Focus::Filter => " filter (typing…) ",
        Focus::List => " filter (press / to edit) ",
    };
    let filter_block = Block::default().borders(Borders::ALL).title(filter_title);
    let filter_para = Paragraph::new(filter).block(filter_block);
    f.render_widget(filter_para, chunks[0]);

    let list_items: Vec<ListItem> = items.iter().map(|e| ListItem::new(render_row(e))).collect();
    let list_title = format!(" history — {} items ", items.len());
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[1], list_state);

    let audio_tag = if audio_only { " [audio-only ON]" } else { "" };
    let hints = format!(
        "↑↓/jk move • PgUp/PgDn jump • / filter • Enter play • a toggle audio{audio_tag} • d delete • q quit"
    );
    let hints_para = Paragraph::new(hints).style(Style::default().fg(Color::DarkGray));
    f.render_widget(hints_para, chunks[2]);
}

fn render_row(e: &HistoryEntry) -> Line<'static> {
    let age = format_age(e.ts_end);
    let pos = fmt_dur(e.position_on_exit);
    let dur = e.duration_secs.map(fmt_dur).unwrap_or_else(|| "--:--".into());
    let pct = e.duration_secs
        .filter(|d| *d > 0.0)
        .map(|d| format!(" {:>3.0}%", 100.0 * e.position_on_exit / d))
        .unwrap_or_default();
    let title = e.title.clone().unwrap_or_else(|| e.url.clone());

    Line::from(vec![
        Span::styled(format!("{age:>6} "), Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("[{pos}/{dur}{pct}] "),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(title),
    ])
}

fn format_age(ts: u64) -> String {
    let now = state::now_secs();
    if ts == 0 || ts > now {
        return "-".into();
    }
    let delta = now - ts;
    if delta < 60 {
        format!("{delta}s")
    } else if delta < 3600 {
        format!("{}m", delta / 60)
    } else if delta < 86_400 {
        format!("{}h", delta / 3600)
    } else if delta < 7 * 86_400 {
        format!("{}d", delta / 86_400)
    } else if delta < 30 * 86_400 {
        format!("{}w", delta / (7 * 86_400))
    } else {
        format!("{}mo", delta / (30 * 86_400))
    }
}

fn fmt_dur(secs: f64) -> String {
    let s = secs as u64;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}
