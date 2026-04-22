use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::process::Command;

#[derive(Deserialize, Debug, Clone)]
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
