use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NamedPlaylist {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NamedVideo {
    pub name: String,
    pub video_id: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Config {
    #[serde(default)]
    pub playlists: Vec<NamedPlaylist>,
    #[serde(default)]
    pub videos: Vec<NamedVideo>,
    /// Legacy single-playlist field. Migrated into `playlists` on load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playlist_url: Option<String>,
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RSTUBE_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        });
    base.join("rstube")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load() -> Config {
    let path = config_path();
    let Ok(bytes) = fs::read_to_string(&path) else { return Config::default(); };
    let mut cfg: Config = toml::from_str(&bytes).unwrap_or_default();
    if let Some(legacy) = cfg.playlist_url.take()
        && cfg.playlists.is_empty()
    {
        cfg.playlists.push(NamedPlaylist { name: "default".into(), url: legacy });
    }
    cfg
}

pub fn save(cfg: &Config) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config dir {}", dir.display()))?;
    let path = config_path();
    let tmp = path.with_extension("toml.tmp");
    let serialized = toml::to_string_pretty(cfg)?;
    fs::write(&tmp, serialized.as_bytes())
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("failed to rename into {}", path.display()))?;
    Ok(())
}

/// Accept a YouTube video id in any of the common shapes and return the bare
/// 11-char id. Accepts:
///   - full watch URL: https://www.youtube.com/watch?v=<id>[&...]
///   - youtu.be short URL: https://youtu.be/<id>[?...]
///   - bare 11-char id
pub fn normalize_video_id(input: &str) -> Result<String> {
    let s = input.trim();
    if s.is_empty() {
        bail!("empty video reference");
    }

    let candidate = if let Some(rest) = s.split_once("v=").map(|(_, r)| r) {
        rest.split(&['&', '#'][..]).next().unwrap_or("").to_owned()
    } else if let Some(rest) = s.strip_prefix("https://youtu.be/")
        .or_else(|| s.strip_prefix("http://youtu.be/"))
    {
        rest.split(&['?', '#', '/'][..]).next().unwrap_or("").to_owned()
    } else if s.starts_with("http://") || s.starts_with("https://") {
        bail!("URL does not look like a YouTube watch URL: {s}");
    } else {
        s.to_owned()
    };

    if candidate.len() != 11
        || !candidate.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!(
            "not a valid YouTube video id: {candidate:?} \
             (expected 11 chars of [A-Za-z0-9_-])"
        );
    }
    Ok(candidate)
}

/// Accept either a full playlist URL or a bare playlist id (PL..., UU..., OLAK5uy...).
/// Returns a canonical URL.
pub fn normalize_playlist(input: &str) -> Result<String> {
    let s = input.trim();
    if s.is_empty() {
        bail!("empty playlist reference");
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        if !s.contains("list=") {
            bail!("URL does not contain a list= parameter: {s}");
        }
        return Ok(s.to_owned());
    }
    // Bare id. Minimal validation: YouTube playlist ids are ASCII, no slashes or spaces.
    if s.contains('/') || s.contains(' ') || s.contains('?') {
        bail!("not a valid playlist id or URL: {s}");
    }
    Ok(format!("https://www.youtube.com/playlist?list={s}"))
}
