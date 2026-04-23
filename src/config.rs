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
pub struct ConfiguredVideo {
    pub video_id: String,
    /// Seconds from the start where playback should begin the first time this
    /// video is played. Auto-extracted from a `t=` query parameter in the URL
    /// you pass to `videos add`, or overridden via `--start`. Zero / absent
    /// means start from the beginning. Ignored once a partial session exists
    /// in history or positions.redb.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_offset_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Config {
    #[serde(default)]
    pub playlists: Vec<NamedPlaylist>,
    #[serde(default)]
    pub videos: Vec<ConfiguredVideo>,
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

/// Parse a video reference into `(id, optional_offset_seconds)`. Accepts:
///   - full watch URL: https://www.youtube.com/watch?v=<id>[&...]
///   - youtu.be short URL: https://youtu.be/<id>[?...]
///   - bare 11-char id
///
/// Additionally extracts the `t=` query parameter if present. `t=` values
/// accept seconds ("178", "178s") and YouTube's compound form ("2m58s",
/// "1h2m3s").
pub fn parse_video_spec(input: &str) -> Result<(String, Option<u64>)> {
    let s = input.trim();
    if s.is_empty() {
        bail!("empty video reference");
    }

    let (id_candidate, query): (String, Option<&str>) =
        if let Some(after_v) = s.split_once("v=").map(|(_, r)| r) {
            let id = after_v.split(&['&', '#'][..]).next().unwrap_or("").to_owned();
            // The whole URL after `?` may contain the t= param, not just after v=.
            let query = s.split_once('?').map(|(_, q)| q);
            (id, query)
        } else if let Some(rest) = s
            .strip_prefix("https://youtu.be/")
            .or_else(|| s.strip_prefix("http://youtu.be/"))
        {
            let id = rest
                .split(&['?', '#', '/'][..])
                .next()
                .unwrap_or("")
                .to_owned();
            let query = rest.split_once('?').map(|(_, q)| q);
            (id, query)
        } else if s.starts_with("http://") || s.starts_with("https://") {
            bail!("URL does not look like a YouTube watch URL: {s}");
        } else {
            (s.to_owned(), None)
        };

    if id_candidate.len() != 11
        || !id_candidate.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!(
            "not a valid YouTube video id: {id_candidate:?} \
             (expected 11 chars of [A-Za-z0-9_-])"
        );
    }

    let offset = match query {
        Some(q) => extract_t_param(q).map(parse_time_spec).transpose()?,
        None => None,
    };
    Ok((id_candidate, offset))
}

/// Pull the `t` parameter's value out of a `&`-separated query string.
/// Returns None if not present.
fn extract_t_param(query: &str) -> Option<&str> {
    for part in query.split('&') {
        if let Some(value) = part.strip_prefix("t=") {
            // Stop at any fragment.
            return Some(value.split('#').next().unwrap_or(value));
        }
    }
    None
}

/// Parse a time specification into seconds. Accepts:
///   - plain seconds: "178", "178s"
///   - compound form (any order/subset of h, m, s): "2m58s", "1h2m3s", "90m"
///   - colon form: "2:58", "1:02:03"
pub fn parse_time_spec(s: &str) -> Result<u64> {
    let s = s.trim();
    if s.is_empty() {
        bail!("empty time spec");
    }

    // Colon form: "2:58" or "1:02:03".
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        let mut total: u64 = 0;
        for part in &parts {
            let n: u64 = part
                .parse()
                .with_context(|| format!("invalid colon-form time spec: {s:?}"))?;
            total = total * 60 + n;
        }
        if parts.len() > 3 {
            bail!("time spec has too many colons: {s:?}");
        }
        return Ok(total);
    }

    // Plain seconds: "178" or "178s".
    if let Some(stripped) = s.strip_suffix('s') {
        if stripped.chars().all(|c| c.is_ascii_digit()) {
            return stripped
                .parse()
                .with_context(|| format!("invalid numeric time spec: {s:?}"));
        }
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return s
            .parse()
            .with_context(|| format!("invalid numeric time spec: {s:?}"));
    }

    // Compound form: h / m / s (each optional, in that order).
    let mut total: u64 = 0;
    let mut num = String::new();
    let mut seen_h = false;
    let mut seen_m = false;
    let mut seen_s = false;
    for ch in s.chars() {
        match ch {
            '0'..='9' => num.push(ch),
            'h' | 'H' => {
                if seen_h || seen_m || seen_s || num.is_empty() {
                    bail!("invalid compound time spec: {s:?}");
                }
                let n: u64 = num.parse().with_context(|| format!("bad hour in {s:?}"))?;
                total += n * 3600;
                num.clear();
                seen_h = true;
            }
            'm' | 'M' => {
                if seen_m || seen_s || num.is_empty() {
                    bail!("invalid compound time spec: {s:?}");
                }
                let n: u64 = num.parse().with_context(|| format!("bad minute in {s:?}"))?;
                total += n * 60;
                num.clear();
                seen_m = true;
            }
            's' | 'S' => {
                if seen_s || num.is_empty() {
                    bail!("invalid compound time spec: {s:?}");
                }
                let n: u64 = num.parse().with_context(|| format!("bad second in {s:?}"))?;
                total += n;
                num.clear();
                seen_s = true;
            }
            _ => bail!("invalid char {ch:?} in time spec: {s:?}"),
        }
    }
    if !num.is_empty() {
        // Trailing bare digits in compound form: treat as seconds if we haven't
        // already seen an explicit "s".
        if seen_s {
            bail!("trailing digits after s suffix: {s:?}");
        }
        let n: u64 = num.parse().with_context(|| format!("bad trailing seconds in {s:?}"))?;
        total += n;
    }
    Ok(total)
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
