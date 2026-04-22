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

use crate::playlist::PlaylistItem;
use crate::state::{self, HistoryEntry};

pub struct Selection {
    pub url: String,
    pub title: Option<String>,
    pub duration_secs: Option<f64>,
    pub audio_only: bool,
}

/// Generic row for the picker. Either source (history or playlist) maps into
/// this; the picker doesn't care where items came from.
pub struct PickerRow {
    pub video_id: String,
    pub url: String,
    pub title: Option<String>,
    pub duration_secs: Option<f64>,
    /// Position in seconds if this item has watch progress, else None.
    pub position_secs: Option<f64>,
    /// Unix timestamp of last play, if any. 0 for never.
    pub last_played: u64,
}

enum Focus {
    List,
    Filter,
}

const MIN_RESUME_SECS: f64 = 10.0;
const RESUME_TAIL_MARGIN_SECS: f64 = 10.0;

fn is_in_progress(e: &HistoryEntry) -> bool {
    if e.position_on_exit < MIN_RESUME_SECS {
        return false;
    }
    match e.duration_secs {
        Some(d) if d > 0.0 => e.position_on_exit < d - RESUME_TAIL_MARGIN_SECS,
        _ => true,
    }
}

/// Resume picker: in-progress history entries. Returns (count-before-picker,
/// selection). `count_before` lets the caller distinguish "nothing to resume"
/// from "user quit the picker".
pub fn run_resume_picker() -> Result<(usize, Option<Selection>)> {
    let rows: Vec<PickerRow> = resume_candidates()
        .into_iter()
        .map(|e| PickerRow {
            video_id: e.video_id,
            url: e.url,
            title: e.title,
            duration_secs: e.duration_secs,
            position_secs: Some(e.position_on_exit),
            last_played: e.ts_end,
        })
        .collect();
    let count = rows.len();
    let sel = run(rows, "resume")?;
    Ok((count, sel))
}

/// History entries eligible for resume: most recent in-progress session per
/// video. Unlike a naive "most recent session", this prefers any session that
/// was actually watched (≥ MIN_RESUME_SECS) over a more recent 0-second one —
/// otherwise a quick accidental reopen clobbers the resumable entry.
fn resume_candidates() -> Vec<HistoryEntry> {
    use std::collections::HashMap;
    let mut by_id: HashMap<String, HistoryEntry> = HashMap::new();
    for e in state::load_all_history() {
        if !is_in_progress(&e) {
            continue;
        }
        by_id
            .entry(e.video_id.clone())
            .and_modify(|existing| {
                if e.ts_end > existing.ts_end {
                    *existing = e.clone();
                }
            })
            .or_insert(e);
    }
    let mut out: Vec<HistoryEntry> = by_id.into_values().collect();
    out.sort_by(|a, b| b.ts_end.cmp(&a.ts_end));
    out
}

/// Picker over playlist items. Caller decides what to include (e.g. unseen-only
/// for `play new`, or everything for `play any`).
pub fn run_playlist_picker(items: Vec<PlaylistItem>) -> Result<Option<Selection>> {
    let rows: Vec<PickerRow> = items
        .into_iter()
        .map(|it| PickerRow {
            url: it.url(),
            video_id: it.id,
            title: it.title,
            duration_secs: it.duration,
            position_secs: None,
            last_played: 0,
        })
        .collect();
    run(rows, "play")
}

/// Minimal chooser over a list of strings. Returns the selected index, or None
/// if the user quit.
pub fn run_playlist_chooser(names: Vec<String>) -> Result<Option<usize>> {
    if names.is_empty() {
        return Ok(None);
    }
    let mut terminal = setup_terminal().context("failed to init terminal")?;
    let result = chooser_loop(&mut terminal, &names);
    restore_terminal(&mut terminal)?;
    result
}

fn chooser_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    names: &[String],
) -> Result<Option<usize>> {
    let mut list_state = ListState::default();
    list_state.select(Some(0));
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(f.area());
            let items: Vec<ListItem> = names.iter().map(|n| ListItem::new(n.clone())).collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" choose playlist "))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("▶ ");
            f.render_stateful_widget(list, chunks[0], &mut list_state);
            let hints = Paragraph::new("↑↓/jk move • Enter select • q quit")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(hints, chunks[1]);
        })?;

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
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            KeyCode::Down | KeyCode::Char('j') => {
                let cur = list_state.selected().unwrap_or(0);
                if cur + 1 < names.len() {
                    list_state.select(Some(cur + 1));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let cur = list_state.selected().unwrap_or(0);
                if cur > 0 {
                    list_state.select(Some(cur - 1));
                }
            }
            KeyCode::Enter => return Ok(list_state.selected()),
            _ => {}
        }
    }
}

fn run(rows: Vec<PickerRow>, label: &str) -> Result<Option<Selection>> {
    if rows.is_empty() {
        return Ok(None);
    }
    let mut terminal = setup_terminal().context("failed to init terminal")?;
    let result = main_loop(&mut terminal, rows, label);
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
    mut rows: Vec<PickerRow>,
    label: &str,
) -> Result<Option<Selection>> {
    let mut filter = String::new();
    let mut focus = Focus::List;
    let mut list_state = ListState::default();
    list_state.select(Some(0));
    let mut audio_only = false;

    loop {
        let filtered = filter_rows(&rows, &filter);
        if list_state.selected().unwrap_or(0) >= filtered.len() && !filtered.is_empty() {
            list_state.select(Some(filtered.len() - 1));
        }
        if filtered.is_empty() {
            list_state.select(None);
        } else if list_state.selected().is_none() {
            list_state.select(Some(0));
        }

        terminal.draw(|f| draw(f, &filtered, &mut list_state, &filter, &focus, audio_only, label))?;

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
                        && let Some(row) = filtered.get(idx)
                    {
                        let id = row.video_id.clone();
                        rows.retain(|r| r.video_id != id);
                        let _ = remove_position(&id);
                    }
                }
                KeyCode::Enter => {
                    if let Some(idx) = list_state.selected()
                        && let Some(row) = filtered.get(idx)
                    {
                        return Ok(Some(Selection {
                            url: row.url.clone(),
                            title: row.title.clone(),
                            duration_secs: row.duration_secs,
                            audio_only,
                        }));
                    }
                }
                _ => {}
            },
        }
    }
}

fn move_selection(state: &mut ListState, items: &[&PickerRow], delta: i32) {
    if items.is_empty() {
        return;
    }
    let len = items.len() as i32;
    let current = state.selected().unwrap_or(0) as i32;
    let next = (current + delta).clamp(0, len - 1);
    state.select(Some(next as usize));
}

fn filter_rows<'a>(rows: &'a [PickerRow], filter: &str) -> Vec<&'a PickerRow> {
    if filter.is_empty() {
        return rows.iter().collect();
    }
    let needle = filter.to_lowercase();
    rows
        .iter()
        .filter(|r| {
            let hay = r.title.as_deref().unwrap_or(&r.url).to_lowercase();
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
    items: &[&PickerRow],
    list_state: &mut ListState,
    filter: &str,
    focus: &Focus,
    audio_only: bool,
    label: &str,
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

    let list_items: Vec<ListItem> = items.iter().map(|r| ListItem::new(render_row(r))).collect();
    let list_title = format!(" {label} — {} items ", items.len());
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

fn render_row(r: &PickerRow) -> Line<'static> {
    let age = format_age(r.last_played);
    let dur = r.duration_secs.map(fmt_dur).unwrap_or_else(|| "--:--".into());
    let progress = match r.position_secs {
        Some(pos) => {
            let pct = r.duration_secs
                .filter(|d| *d > 0.0)
                .map(|d| format!(" {:>3.0}%", 100.0 * pos / d))
                .unwrap_or_default();
            format!("[{}/{dur}{pct}] ", fmt_dur(pos))
        }
        None => format!("[{dur}] "),
    };
    let title = r.title.clone().unwrap_or_else(|| r.url.clone());

    Line::from(vec![
        Span::styled(format!("{age:>6} "), Style::default().fg(Color::DarkGray)),
        Span::styled(progress, Style::default().fg(Color::Cyan)),
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
