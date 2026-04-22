use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlaylistItem {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
}

impl PlaylistItem {
    pub fn url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.id)
    }
}

/// Fetch a single video's metadata via `yt-dlp -j <watch-url>`. Unlike the
/// playlist fetch, this is a full per-video call — 1-3s typical — and returns
/// `title` and `duration` fields populated.
pub fn fetch_video(video_id: &str) -> Result<PlaylistItem> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let output = Command::new("yt-dlp")
        .args(["-j", "--no-playlist", &url])
        .output()
        .context("failed to run yt-dlp")?;
    if !output.status.success() {
        bail!(
            "yt-dlp video fetch failed for {video_id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // yt-dlp -j outputs one big JSON object. Parse the fields we want; the
    // rest are ignored.
    #[derive(Deserialize)]
    struct RawVideo {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        duration: Option<f64>,
    }
    let raw: RawVideo = serde_json::from_slice(&output.stdout)
        .context("failed to parse yt-dlp video JSON")?;
    Ok(PlaylistItem {
        id: raw.id.unwrap_or_else(|| video_id.to_owned()),
        title: raw.title,
        duration: raw.duration,
    })
}

/// Fetch playlist contents via `yt-dlp --flat-playlist -j <url>`. One JSON
/// document per line. `--flat-playlist` skips per-video fetches, so even long
/// playlists return in ~1s.
pub fn fetch(url: &str) -> Result<Vec<PlaylistItem>> {
    let output = Command::new("yt-dlp")
        .args(["-j", "--flat-playlist", url])
        .output()
        .context("failed to run yt-dlp")?;
    if !output.status.success() {
        bail!(
            "yt-dlp playlist fetch failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut items = Vec::new();
    for line in output.stdout.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let item: PlaylistItem = serde_json::from_slice(line)
            .context("failed to parse yt-dlp JSON line")?;
        items.push(item);
    }
    Ok(items)
}
