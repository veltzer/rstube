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
    state_dir().join("positions.redb")
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

use redb::ReadableTable;

// Positions are stored in a redb key/value database. Keys are video ids
// (strings); values are JSON-serialized `Position` records (bytes). Using
// JSON as the value encoding keeps the file trivially inspectable with
// `redb dump` and lets us add Position fields without a schema migration.
const POSITIONS_TABLE: redb::TableDefinition<&str, &[u8]> =
    redb::TableDefinition::new("positions");

fn open_positions_db() -> Result<redb::Database> {
    ensure_state_dir()?;
    let path = positions_path();
    redb::Database::create(&path)
        .with_context(|| format!("failed to open positions db at {}", path.display()))
}

pub fn load_positions() -> HashMap<String, Position> {
    let Ok(db) = open_positions_db() else { return HashMap::new(); };
    let Ok(txn) = db.begin_read() else { return HashMap::new(); };
    // Missing table on a fresh db is not an error — just no entries yet.
    let Ok(table) = txn.open_table(POSITIONS_TABLE) else { return HashMap::new(); };
    let mut out = HashMap::new();
    let Ok(iter) = table.iter() else { return out; };
    for row in iter.flatten() {
        let (k, v) = row;
        if let Ok(pos) = serde_json::from_slice::<Position>(v.value()) {
            out.insert(k.value().to_owned(), pos);
        }
    }
    out
}

pub fn upsert_position(video_id: &str, pos: Position) -> Result<()> {
    let db = open_positions_db()?;
    let txn = db.begin_write().context("begin_write positions")?;
    {
        let mut table = txn.open_table(POSITIONS_TABLE).context("open positions table")?;
        let bytes = serde_json::to_vec(&pos)?;
        table
            .insert(video_id, bytes.as_slice())
            .context("insert position")?;
    }
    txn.commit().context("commit positions")?;
    Ok(())
}

pub fn get_position(video_id: &str) -> Option<Position> {
    let db = open_positions_db().ok()?;
    let txn = db.begin_read().ok()?;
    let table = txn.open_table(POSITIONS_TABLE).ok()?;
    let row = table.get(video_id).ok()??;
    serde_json::from_slice::<Position>(row.value()).ok()
}

pub fn delete_position(video_id: &str) -> Result<()> {
    let db = open_positions_db()?;
    let txn = db.begin_write().context("begin_write positions")?;
    {
        let mut table = txn.open_table(POSITIONS_TABLE).context("open positions table")?;
        table.remove(video_id).context("remove position")?;
    }
    txn.commit().context("commit positions")?;
    Ok(())
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

/// Search every cached playlist for a video by id. Useful for surfacing a
/// title when we only have a position entry and no history row.
pub fn find_in_playlist_cache(video_id: &str) -> Option<PlaylistItem> {
    let cache = load_playlist_cache_file();
    for entry in cache.entries.values() {
        for item in &entry.items {
            if item.id == video_id {
                return Some(item.clone());
            }
        }
    }
    None
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

/// Load history sessions, merging the phase-1 (session-start) and phase-2
/// (session-end) lines of each session. A session is identified by
/// `(video_id, ts_start)`; the line with the greater `ts_end` wins, so a
/// phase-2 line (which has the real `ts_end` and `position_on_exit`) always
/// replaces its matching phase-1 line. Sessions that only ever got a phase-1
/// line (mpv killed by power loss, SIGKILL, etc.) are retained as-is.
pub fn load_history_sessions() -> Vec<HistoryEntry> {
    let mut by_key: HashMap<(String, u64), HistoryEntry> = HashMap::new();
    for e in load_all_history() {
        let key = (e.video_id.clone(), e.ts_start);
        by_key
            .entry(key)
            .and_modify(|existing| {
                if e.ts_end >= existing.ts_end {
                    *existing = e.clone();
                }
            })
            .or_insert(e);
    }
    let mut out: Vec<HistoryEntry> = by_key.into_values().collect();
    out.sort_by_key(|e| e.ts_start);
    out
}

/// Load all history records in file order. Silently skips malformed lines.
pub fn load_all_history() -> Vec<HistoryEntry> {
    let path = history_path();
    let Ok(contents) = fs::read_to_string(&path) else { return Vec::new(); };
    let mut out = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) {
            out.push(entry);
        }
    }
    out
}

