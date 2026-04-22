mod config;
mod mpv;
mod playlist;
mod state;
mod tui;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "rstube")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Resume and replay YouTube videos via mpv, with position tracking")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show recent playback history
    History {
        /// Max entries to show (most recent first)
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Open TUI picker to resume an in-progress video from history
    Resume,
    /// Open TUI picker to play a never-before-watched video from the playlist
    Playnew {
        /// Bypass the playlist cache and refetch from YouTube
        #[arg(long)]
        refresh: bool,
    },
    /// Manage the configured YouTube playlist (used by `playnew`)
    Playlist {
        #[command(subcommand)]
        action: PlaylistAction,
    },
    /// Print full build/version info (git sha, rustc, build time)
    Version,
}

#[derive(Subcommand)]
enum PlaylistAction {
    /// Set the playlist URL or bare playlist id
    Set { url_or_id: String },
    /// Print the configured playlist
    Show,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            print_version();
            Ok(())
        }
        Commands::History { limit } => show_history(limit),
        Commands::Resume => run_resume(),
        Commands::Playnew { refresh } => run_playnew(refresh),
        Commands::Playlist { action } => run_playlist(action),
    }
}

fn run_resume() -> Result<()> {
    let Some(sel) = tui::run_resume_picker()? else {
        return Ok(());
    };
    ensure_tool("mpv")?;
    mpv::play(mpv::PlayRequest {
        url: &sel.url,
        title: sel.title.as_deref(),
        duration_secs: sel.duration_secs,
        audio_only: sel.audio_only,
    })
}

const PLAYLIST_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

fn run_playnew(refresh: bool) -> Result<()> {
    let cfg = config::load();
    let Some(url) = cfg.playlist_url else {
        bail!(
            "no playlist configured — set one with `rstube playlist set <url-or-id>` \
             (config: {})",
            config::config_path().display()
        );
    };

    let items = if refresh {
        fetch_and_cache(&url)?
    } else {
        match state::load_playlist_cache(&url) {
            Some(entry) if state::now_secs().saturating_sub(entry.fetched_at) < PLAYLIST_CACHE_TTL_SECS => {
                let age_mins = state::now_secs().saturating_sub(entry.fetched_at) / 60;
                eprintln!("Using cached playlist ({} items, {}m old — rerun with --refresh to refetch).", entry.items.len(), age_mins);
                entry.items
            }
            _ => fetch_and_cache(&url)?,
        }
    };

    if items.is_empty() {
        bail!("playlist returned no items: {url}");
    }

    let seen = state::played_video_ids();
    let unseen: Vec<playlist::PlaylistItem> = items
        .into_iter()
        .filter(|it| !seen.contains(&it.id))
        .collect();
    if unseen.is_empty() {
        eprintln!("No new videos — every item in the playlist is already in your history.");
        eprintln!("(try `rstube playnew --refresh` to refetch the playlist)");
        return Ok(());
    }

    let Some(sel) = tui::run_playnew_picker(unseen)? else {
        return Ok(());
    };
    ensure_tool("mpv")?;
    mpv::play(mpv::PlayRequest {
        url: &sel.url,
        title: sel.title.as_deref(),
        duration_secs: sel.duration_secs,
        audio_only: sel.audio_only,
    })
}

fn fetch_and_cache(url: &str) -> Result<Vec<playlist::PlaylistItem>> {
    ensure_tool("yt-dlp")?;
    eprintln!("Fetching playlist…");
    let items = playlist::fetch(url)?;
    if let Err(e) = state::save_playlist_cache(url, &items) {
        eprintln!("warning: failed to save playlist cache: {e}");
    }
    Ok(items)
}

fn run_playlist(action: PlaylistAction) -> Result<()> {
    match action {
        PlaylistAction::Set { url_or_id } => {
            let url = config::normalize_playlist(&url_or_id)?;
            let mut cfg = config::load();
            cfg.playlist_url = Some(url.clone());
            config::save(&cfg)?;
            println!("playlist set to {url}");
            println!("(stored in {})", config::config_path().display());
            Ok(())
        }
        PlaylistAction::Show => {
            let cfg = config::load();
            match cfg.playlist_url {
                Some(url) => println!("{url}"),
                None => {
                    println!("(no playlist configured)");
                    println!("(config path: {})", config::config_path().display());
                }
            }
            Ok(())
        }
    }
}

fn show_history(limit: usize) -> Result<()> {
    let path = state::history_path();
    if !path.exists() {
        println!("(no history yet — path: {})", path.display());
        return Ok(());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let lines: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(limit);
    for line in &lines[start..] {
        let entry: state::HistoryEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let title = entry.title.as_deref().unwrap_or(&entry.url);
        let pos = fmt_dur(entry.position_on_exit);
        let dur = entry.duration_secs.map(fmt_dur).unwrap_or_else(|| "--:--".into());
        let pct = entry.duration_secs
            .filter(|d| *d > 0.0)
            .map(|d| format!(" ({:.0}%)", 100.0 * entry.position_on_exit / d))
            .unwrap_or_default();
        println!("[{pos}/{dur}{pct}] {title}");
    }
    Ok(())
}

fn print_version() {
    println!("rstube {} by {}", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));
    println!("GIT_DESCRIBE: {}", env!("GIT_DESCRIBE"));
    println!("GIT_SHA: {}", env!("GIT_SHA"));
    println!("GIT_BRANCH: {}", env!("GIT_BRANCH"));
    println!("GIT_DIRTY: {}", env!("GIT_DIRTY"));
    println!("RUSTC_SEMVER: {}", env!("RUSTC_SEMVER"));
    println!("RUST_EDITION: {}", env!("RUST_EDITION"));
    println!("BUILD_TIMESTAMP: {}", env!("BUILD_TIMESTAMP"));
}

fn ensure_tool(name: &str) -> Result<()> {
    which(name).with_context(|| format!("{name} not found in PATH — please install it"))
}

fn which(name: &str) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .stdout(Stdio::null())
        .status()?;
    if !status.success() {
        bail!("{name} not found");
    }
    Ok(())
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
