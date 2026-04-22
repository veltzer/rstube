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
    /// Open TUI picker to play a never-before-watched video from a playlist
    Playnew {
        /// Bypass the playlist cache and refetch from YouTube
        #[arg(long)]
        refresh: bool,
        /// Open a chooser to select which playlist to use
        #[arg(long)]
        pick: bool,
    },
    /// Manage the configured YouTube playlists (used by `playnew`)
    Playlists {
        #[command(subcommand)]
        action: PlaylistsAction,
    },
    /// Print full build/version info (git sha, rustc, build time)
    Version,
}

#[derive(Subcommand)]
enum PlaylistsAction {
    /// Add a playlist under a short name
    Add { name: String, url_or_id: String },
    /// Remove a playlist by name
    Remove { name: String },
    /// List configured playlists in order
    List,
    /// Print a single playlist's URL by name
    Show { name: String },
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
        Commands::Playnew { refresh, pick } => run_playnew(refresh, pick),
        Commands::Playlists { action } => run_playlists(action),
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

fn run_playnew(refresh: bool, pick: bool) -> Result<()> {
    let cfg = config::load();
    if cfg.playlists.is_empty() {
        bail!(
            "no playlists configured — add one with `rstube playlists add <name> <url-or-id>` \
             (config: {})",
            config::config_path().display()
        );
    }

    let seen = state::played_video_ids();

    let (chosen_name, unseen) = if pick {
        let names: Vec<String> = cfg.playlists.iter().map(|p| p.name.clone()).collect();
        let Some(idx) = tui::run_playlist_chooser(names)? else {
            return Ok(());
        };
        let pl = &cfg.playlists[idx];
        let items = load_playlist_items(&pl.url, refresh)?;
        let unseen = filter_unseen(items, &seen);
        if unseen.is_empty() {
            eprintln!("No new videos in playlist \"{}\".", pl.name);
            return Ok(());
        }
        (pl.name.clone(), unseen)
    } else {
        let mut found: Option<(String, Vec<playlist::PlaylistItem>)> = None;
        for pl in &cfg.playlists {
            let items = load_playlist_items(&pl.url, refresh)?;
            let unseen = filter_unseen(items, &seen);
            if !unseen.is_empty() {
                found = Some((pl.name.clone(), unseen));
                break;
            }
            eprintln!("Playlist \"{}\": no new videos, skipping.", pl.name);
        }
        let Some(found) = found else {
            eprintln!("No new videos in any configured playlist.");
            eprintln!("(try `rstube playnew --refresh` to refetch, or `--pick` to choose a playlist)");
            return Ok(());
        };
        found
    };

    eprintln!("Playing from \"{chosen_name}\" ({} unseen).", unseen.len());

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

fn load_playlist_items(url: &str, refresh: bool) -> Result<Vec<playlist::PlaylistItem>> {
    if refresh {
        return fetch_and_cache(url);
    }
    match state::load_playlist_cache(url) {
        Some(entry) if state::now_secs().saturating_sub(entry.fetched_at) < PLAYLIST_CACHE_TTL_SECS => {
            let age_mins = state::now_secs().saturating_sub(entry.fetched_at) / 60;
            eprintln!(
                "Using cached playlist for {url} ({} items, {}m old).",
                entry.items.len(),
                age_mins
            );
            Ok(entry.items)
        }
        _ => fetch_and_cache(url),
    }
}

fn filter_unseen(
    items: Vec<playlist::PlaylistItem>,
    seen: &std::collections::HashSet<String>,
) -> Vec<playlist::PlaylistItem> {
    items.into_iter().filter(|it| !seen.contains(&it.id)).collect()
}

fn fetch_and_cache(url: &str) -> Result<Vec<playlist::PlaylistItem>> {
    ensure_tool("yt-dlp")?;
    eprintln!("Fetching playlist {url}…");
    let items = playlist::fetch(url)?;
    if let Err(e) = state::save_playlist_cache(url, &items) {
        eprintln!("warning: failed to save playlist cache: {e}");
    }
    Ok(items)
}

fn run_playlists(action: PlaylistsAction) -> Result<()> {
    match action {
        PlaylistsAction::Add { name, url_or_id } => {
            let url = config::normalize_playlist(&url_or_id)?;
            let mut cfg = config::load();
            if cfg.playlists.iter().any(|p| p.name == name) {
                bail!("playlist named \"{name}\" already exists");
            }
            cfg.playlists.push(config::NamedPlaylist { name: name.clone(), url: url.clone() });
            config::save(&cfg)?;
            println!("added \"{name}\" → {url}");
            println!("(stored in {})", config::config_path().display());
            Ok(())
        }
        PlaylistsAction::Remove { name } => {
            let mut cfg = config::load();
            let before = cfg.playlists.len();
            cfg.playlists.retain(|p| p.name != name);
            if cfg.playlists.len() == before {
                bail!("no playlist named \"{name}\"");
            }
            config::save(&cfg)?;
            println!("removed \"{name}\"");
            Ok(())
        }
        PlaylistsAction::List => {
            let cfg = config::load();
            if cfg.playlists.is_empty() {
                println!("(no playlists configured)");
                println!("(config path: {})", config::config_path().display());
                return Ok(());
            }
            for (i, pl) in cfg.playlists.iter().enumerate() {
                println!("{:>2}. {}  {}", i + 1, pl.name, pl.url);
            }
            Ok(())
        }
        PlaylistsAction::Show { name } => {
            let cfg = config::load();
            let Some(pl) = cfg.playlists.iter().find(|p| p.name == name) else {
                bail!("no playlist named \"{name}\"");
            };
            println!("{}", pl.url);
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
