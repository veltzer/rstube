use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::playlist::PlaylistItem;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Position {
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub ts_start: u64,
    pub ts_end: u64,
    pub video_id: String,
    pub url: String,
    pub title: Option<String>,
    pub duration_secs: Option<f64>,
    pub position_on_exit: f64,
    pub audio_only: bool,
}

pub fn state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RSTUBE_STATE_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".local/state")
        });
    base.join("rstube")
}

pub fn positions_path() -> PathBuf {
    state_dir().join("positions.json")
}

pub fn history_path() -> PathBuf {
    state_dir().join("history.jsonl")
}

pub fn playlist_cache_path() -> PathBuf {
    state_dir().join("playlist_cache.json")
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn ensure_state_dir() -> Result<PathBuf> {
    let dir = state_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create state dir {}", dir.display()))?;
    Ok(dir)
}

pub fn load_positions() -> HashMap<String, Position> {
    let path = positions_path();
    let Ok(bytes) = fs::read(&path) else { return HashMap::new(); };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Atomic write: serialize to a tmp file in the same dir, then rename over the
/// target. Prevents corruption if we're killed mid-write.
pub fn save_positions(positions: &HashMap<String, Position>) -> Result<()> {
    ensure_state_dir()?;
    let path = positions_path();
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(positions)?;
    fs::write(&tmp, &bytes)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("failed to rename into {}", path.display()))?;
    Ok(())
}

pub fn upsert_position(video_id: &str, pos: Position) -> Result<()> {
    let mut positions = load_positions();
    positions.insert(video_id.to_owned(), pos);
    save_positions(&positions)
}

pub fn get_position(video_id: &str) -> Option<Position> {
    load_positions().get(video_id).cloned()
}

pub fn append_history(entry: &HistoryEntry) -> Result<()> {
    ensure_state_dir()?;
    let path = history_path();
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let line = serde_json::to_string(entry)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// Extract a YouTube video id from a URL. Returns None if no recognizable id
/// is present (in which case we fall back to the URL itself as a stable key).
pub fn video_id_from_url(url: &str) -> Option<String> {
    if let Some(rest) = url.split_once("v=").map(|(_, r)| r) {
        return Some(rest.split(&['&', '#'][..]).next()?.to_owned());
    }
    if let Some(rest) = url.strip_prefix("https://youtu.be/") {
        return Some(rest.split(&['?', '#', '/'][..]).next()?.to_owned());
    }
    if let Some(rest) = url.strip_prefix("http://youtu.be/") {
        return Some(rest.split(&['?', '#', '/'][..]).next()?.to_owned());
    }
    None
}

/// Stable key for a URL: prefer the video id, else the URL.
pub fn url_key(url: &str) -> String {
    video_id_from_url(url).unwrap_or_else(|| url.to_owned())
}

#[allow(dead_code)]
pub fn path_exists(p: &Path) -> bool {
    p.exists()
}

/// Set of every video_id that appears in history.jsonl, regardless of how far
/// it was watched. "Strict" notion of played: any session at all counts.
pub fn played_video_ids() -> std::collections::HashSet<String> {
    let path = history_path();
    let Ok(contents) = fs::read_to_string(&path) else { return std::collections::HashSet::new(); };
    let mut ids = std::collections::HashSet::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) {
            ids.insert(entry.video_id);
        }
    }
    ids
}

#[derive(Serialize, Deserialize, Debug)]
struct PlaylistCacheFile {
    #[serde(default)]
    entries: HashMap<String, PlaylistCacheEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlaylistCacheEntry {
    pub fetched_at: u64,
    pub items: Vec<PlaylistItem>,
}

fn load_playlist_cache_file() -> PlaylistCacheFile {
    let path = playlist_cache_path();
    let Ok(bytes) = fs::read(&path) else {
        return PlaylistCacheFile { entries: HashMap::new() };
    };
    serde_json::from_slice(&bytes).unwrap_or(PlaylistCacheFile { entries: HashMap::new() })
}

pub fn load_playlist_cache(url: &str) -> Option<PlaylistCacheEntry> {
    load_playlist_cache_file().entries.remove(url)
}

pub fn save_playlist_cache(url: &str, items: &[PlaylistItem]) -> Result<()> {
    ensure_state_dir()?;
    let mut cache = load_playlist_cache_file();
    cache.entries.insert(
        url.to_owned(),
        PlaylistCacheEntry {
            fetched_at: now_secs(),
            items: items.to_vec(),
        },
    );
    let path = playlist_cache_path();
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&cache)?;
    fs::write(&tmp, &bytes)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("failed to rename into {}", path.display()))?;
    Ok(())
}

/// Load all history records, deduplicate by video_id keeping the most recent
/// session, and return them sorted newest-first. Silently skips malformed lines.
pub fn load_history_deduped() -> Vec<HistoryEntry> {
    let path = history_path();
    let Ok(contents) = fs::read_to_string(&path) else { return Vec::new(); };
    let mut by_id: HashMap<String, HistoryEntry> = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) else { continue };
        by_id
            .entry(entry.video_id.clone())
            .and_modify(|existing| {
                if entry.ts_end > existing.ts_end {
                    *existing = entry.clone();
                }
            })
            .or_insert(entry);
    }
    let mut out: Vec<HistoryEntry> = by_id.into_values().collect();
    out.sort_by(|a, b| b.ts_end.cmp(&a.ts_end));
    out
}
