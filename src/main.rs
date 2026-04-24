mod config;
mod mpv;
mod playlist;
mod state;
mod tui;

use anyhow::{Context, Result, bail};
use chrono::TimeZone;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "rstube")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Play YouTube videos via mpv, with position tracking and playlist-aware pickers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect playback history
    History {
        #[command(subcommand)]
        action: HistoryAction,
    },
    /// Play videos: resume a partial, pick a new one, or browse anything
    Play {
        #[command(subcommand)]
        action: PlayAction,
    },
    /// Print lists of videos by category (finished, partial, new)
    Show {
        #[command(subcommand)]
        action: ShowAction,
    },
    /// Forget a watched video so it re-appears as "new"
    Forget {
        #[command(subcommand)]
        action: ForgetAction,
    },
    /// Manage the configured YouTube playlists (used by `play new`)
    Playlists {
        #[command(subcommand)]
        action: PlaylistsAction,
    },
    /// Manage individually configured YouTube videos (merged into play/show pools)
    Videos {
        #[command(subcommand)]
        action: VideosAction,
    },
    /// Generate shell completion scripts
    Complete {
        /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
        shell: Shell,
    },
    /// Install missing runtime dependencies (mpv, yt-dlp)
    InstallDeps,
    /// Print full build/version info (git sha, rustc, build time)
    Version,
}

#[derive(Subcommand)]
enum PlayAction {
    /// Pick a partially-watched video from history to resume
    Partial {
        /// Show mpv's terminal status line and verbose log output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Pick a never-before-watched video from anywhere in your configured playlists and videos
    New {
        /// Bypass the playlist cache and refetch from YouTube
        #[arg(long)]
        refresh: bool,
        /// Show mpv's terminal status line and verbose log output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Pick any video across all configured playlists, ignoring watch history
    Any {
        /// Bypass the playlist cache and refetch from YouTube
        #[arg(long)]
        refresh: bool,
        /// Show mpv's terminal status line and verbose log output
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum HistoryAction {
    /// Show recent playback history
    Show {
        /// Max entries to show (most recent first)
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Include session start/end timestamps
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum ShowAction {
    /// List videos watched to (near) the end
    Finished {
        /// Include timing/percentage (default: just id and title)
        #[arg(short, long)]
        details: bool,
    },
    /// List videos partially watched — same set `play partial` offers
    Partial,
    /// List videos in configured playlists you haven't started yet
    New {
        /// Bypass the playlist cache and refetch from YouTube
        #[arg(long)]
        refresh: bool,
    },
}

#[derive(Subcommand)]
enum ForgetAction {
    /// Pick a partial video and forget it (removes history + saved position)
    Partial,
    /// Pick a finished video and forget it (removes history + saved position)
    Finished,
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
    /// Fetch playlists from YouTube and update the local cache
    Fetch {
        /// Playlist name to fetch (omit to fetch all configured playlists)
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum VideosAction {
    /// Add a single video. url-or-id accepts a full watch URL, a youtu.be
    /// short URL (both may include ?t=N), or a bare 11-char video id. A
    /// non-zero offset flags the video as "partial" immediately. By default
    /// rstube looks up the video via yt-dlp to validate it and cache its
    /// title/duration — pass --no-fetch to skip that.
    Add {
        url_or_id: String,
        /// Start offset (seconds, "1m23s", "1:23", etc). Overrides any `t=`
        /// in the URL.
        #[arg(long)]
        start: Option<String>,
        /// Skip the yt-dlp lookup at add time
        #[arg(long)]
        no_fetch: bool,
    },
    /// Remove a configured video (by url, short url, or bare 11-char id)
    Remove { url_or_id: String },
    /// List configured videos in order
    List,
    /// Fetch title+duration from YouTube and update the local cache
    Fetch {
        /// Video url-or-id to fetch (omit to fetch all configured videos)
        url_or_id: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            print_version();
            Ok(())
        }
        Commands::History { action } => run_history(action),
        Commands::Play { action } => run_play(action),
        Commands::Show { action } => run_show(action),
        Commands::Forget { action } => run_forget(action),
        Commands::Playlists { action } => run_playlists(action),
        Commands::Videos { action } => run_videos(action),
        Commands::Complete { shell } => {
            print_completions(shell);
            Ok(())
        }
        Commands::InstallDeps => run_install_deps(),
    }
}

fn run_history(action: HistoryAction) -> Result<()> {
    match action {
        HistoryAction::Show { limit, verbose } => show_history(limit, verbose),
    }
}

fn run_play(action: PlayAction) -> Result<()> {
    match action {
        PlayAction::Partial { verbose } => run_play_partial(verbose),
        PlayAction::New { refresh, verbose } => run_play_new(refresh, verbose),
        PlayAction::Any { refresh, verbose } => run_play_any(refresh, verbose),
    }
}

fn run_play_partial(verbose: bool) -> Result<()> {
    let (count, sel) = tui::run_partial_picker()?;
    if count == 0 {
        eprintln!("Nothing to resume — no videos with ≥10s of watch time in history.");
        eprintln!("(use `rstube play new` or `rstube play any` to start a new video)");
        return Ok(());
    }
    let Some(sel) = sel else {
        return Ok(());
    };
    ensure_tool("mpv")?;
    mpv::play(mpv::PlayRequest {
        url: &sel.url,
        title: sel.title.as_deref(),
        duration_secs: sel.duration_secs,
        audio_only: sel.audio_only,
        verbose,
    })
}

const PLAYLIST_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// Merge all configured playlists into one deduped list (by video id),
/// preserving first-occurrence order. Bails if no playlists are configured
/// or every playlist is empty.
fn load_merged_playlists(refresh: bool) -> Result<Vec<playlist::PlaylistItem>> {
    let cfg = config::load();
    if cfg.playlists.is_empty() && cfg.videos.is_empty() {
        bail!(
            "nothing configured — add a playlist with `rstube playlists add <name> <url-or-id>` \
             or a single video with `rstube videos add <name> <url-or-id>` \
             (config: {})",
            config::config_path().display()
        );
    }
    let mut merged: Vec<playlist::PlaylistItem> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for pl in &cfg.playlists {
        let items = load_playlist_items(&pl.url, refresh)?;
        for it in items {
            if seen_ids.insert(it.id.clone()) {
                merged.push(it);
            }
        }
    }
    for v in &cfg.videos {
        if seen_ids.contains(&v.video_id) {
            continue;
        }
        match load_configured_video(&v.video_id, refresh) {
            Ok(item) => {
                seen_ids.insert(item.id.clone());
                merged.push(item);
            }
            Err(e) => {
                eprintln!("warning: skipping configured video {}: {e}", v.video_id);
            }
        }
    }
    if merged.is_empty() {
        bail!("all configured playlists and videos are empty");
    }
    Ok(merged)
}

fn run_play_any(refresh: bool, verbose: bool) -> Result<()> {
    let merged = load_merged_playlists(refresh)?;
    let cfg = config::load();

    eprintln!("{} total videos across {} playlists.", merged.len(), cfg.playlists.len());

    let Some(sel) = tui::run_playlist_picker(merged)? else {
        return Ok(());
    };
    ensure_tool("mpv")?;
    mpv::play(mpv::PlayRequest {
        url: &sel.url,
        title: sel.title.as_deref(),
        duration_secs: sel.duration_secs,
        audio_only: sel.audio_only,
        verbose,
    })
}

fn run_play_new(refresh: bool, verbose: bool) -> Result<()> {
    let merged = load_merged_playlists(refresh)?;
    let seen = state::played_video_ids();
    let unseen = filter_unseen(merged, &seen);
    if unseen.is_empty() {
        eprintln!("No new videos — every item in your playlists/videos is in history.");
        eprintln!("(try `rstube play new --refresh` to refetch)");
        return Ok(());
    }

    eprintln!("{} unseen videos.", unseen.len());

    let Some(sel) = tui::run_playlist_picker(unseen)? else {
        return Ok(());
    };
    ensure_tool("mpv")?;
    mpv::play(mpv::PlayRequest {
        url: &sel.url,
        title: sel.title.as_deref(),
        duration_secs: sel.duration_secs,
        audio_only: sel.audio_only,
        verbose,
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

fn run_forget(action: ForgetAction) -> Result<()> {
    let (count, sel) = match action {
        ForgetAction::Partial => tui::run_partial_picker()?,
        ForgetAction::Finished => tui::run_finished_picker()?,
    };
    if count == 0 {
        let label = match action {
            ForgetAction::Partial => "partial",
            ForgetAction::Finished => "finished",
        };
        eprintln!("Nothing to forget — no {label} videos.");
        return Ok(());
    }
    let Some(sel) = sel else {
        return Ok(());
    };
    let title = sel.title.as_deref().unwrap_or(&sel.url);
    let removed = state::forget_history(&sel.video_id)?;
    match state::delete_position(&sel.video_id) {
        Ok(()) => {}
        Err(e) => eprintln!("warning: failed to remove position for {}: {e}", sel.video_id),
    }
    println!(
        "Forgot \"{title}\" ({}): removed {removed} history line{} and cleared saved position.",
        sel.video_id,
        if removed == 1 { "" } else { "s" }
    );
    Ok(())
}

fn run_playlists(action: PlaylistsAction) -> Result<()> {
    match action {
        PlaylistsAction::Add { name, url_or_id } => {
            let url = config::normalize_playlist(&url_or_id)?;
            let mut cfg = config::load();
            if cfg.playlists.iter().any(|p| p.name == name) {
                bail!("playlist named \"{name}\" already exists");
            }
            if let Some(existing) = cfg.playlists.iter().find(|p| p.url == url) {
                bail!(
                    "playlist URL already configured as \"{}\" \
                     — remove it first if you want to re-add",
                    existing.name
                );
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
        PlaylistsAction::Fetch { name } => {
            let cfg = config::load();
            if cfg.playlists.is_empty() {
                bail!("no playlists configured — add one with `rstube playlists add <name> <url-or-id>`");
            }
            let targets: Vec<&config::NamedPlaylist> = match name {
                Some(n) => {
                    let Some(pl) = cfg.playlists.iter().find(|p| p.name == n) else {
                        bail!("no playlist named \"{n}\"");
                    };
                    vec![pl]
                }
                None => cfg.playlists.iter().collect(),
            };
            for pl in targets {
                let items = fetch_and_cache(&pl.url)?;
                println!("{}: {} items cached", pl.name, items.len());
            }
            Ok(())
        }
    }
}

fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(shell, &mut cmd, "rstube", &mut buf);
    let script = String::from_utf8(buf).expect("completion script should be UTF-8");
    match shell {
        Shell::Bash => print!("{}", inject_bash_playlist_completions(&script)),
        _ => print!("{script}"),
    }
}

/// Inject bash completion for configured playlist names and video ids.
/// Values are read from the config TOML at tab time, scoped by section so
/// `playlists remove <TAB>` suggests only playlist names and
/// `videos remove <TAB>` suggests only video ids.
fn inject_bash_playlist_completions(script: &str) -> String {
    let helper = r#"
_rstube_field_in_section() {
    # args: $1 = section name (without brackets), $2 = TOML field name
    local cfg="${RSTUBE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/rstube}/config.toml"
    [[ -f "$cfg" ]] || return
    awk -v want="[[$1]]" -v field="$2" '
        /^\[\[/ { in_section = ($0 == want); next }
        /^\[/   { in_section = 0; next }
        in_section && $1 == field {
            match($0, /"[^"]*"/)
            if (RSTART > 0) print substr($0, RSTART+1, RLENGTH-2)
        }
    ' "$cfg"
}
_rstube_playlist_names() { _rstube_field_in_section playlists name; }
_rstube_video_ids()      { _rstube_field_in_section videos video_id; }
"#;

    let playlist_targets = [
        "rstube__subcmd__playlists__subcmd__show)",
        "rstube__subcmd__playlists__subcmd__remove)",
        "rstube__subcmd__playlists__subcmd__fetch)",
    ];
    let video_targets = [
        "rstube__subcmd__videos__subcmd__remove)",
        "rstube__subcmd__videos__subcmd__fetch)",
    ];

    let needle = "COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )";
    let playlist_replacement = "if [[ ${cur} != -* ]] ; then\n                    COMPREPLY=( $(compgen -W \"$(_rstube_playlist_names)\" -- \"${cur}\") )\n                else\n                    COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n                fi";
    let video_replacement = "if [[ ${cur} != -* ]] ; then\n                    COMPREPLY=( $(compgen -W \"$(_rstube_video_ids)\" -- \"${cur}\") )\n                else\n                    COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n                fi";

    let mut result = script.to_string();
    for (targets, replacement) in [
        (&playlist_targets[..], playlist_replacement),
        (&video_targets[..], video_replacement),
    ] {
        for target in targets {
            let Some(section_start) = result.find(target) else { continue };
            let after_start = section_start + target.len();
            let section_len = result[after_start..]
                .find("\n        rstube__subcmd__")
                .unwrap_or(result.len() - after_start);
            let section_end = after_start + section_len;
            let section_slice = &result[section_start..section_end];
            if let Some(rel_pos) = section_slice.find(needle) {
                let abs_pos = section_start + rel_pos;
                result.replace_range(abs_pos..abs_pos + needle.len(), replacement);
            }
        }
    }
    format!("{helper}{result}")
}

fn run_videos(action: VideosAction) -> Result<()> {
    match action {
        VideosAction::Add { url_or_id, start, no_fetch } => {
            let (video_id, url_offset) = config::parse_video_spec(&url_or_id)?;
            // Explicit --start wins over a URL's t= param.
            let offset = match start {
                Some(s) => Some(config::parse_time_spec(&s)?),
                None => url_offset,
            };
            let mut cfg = config::load();
            if cfg.videos.iter().any(|v| v.video_id == video_id) {
                bail!(
                    "video id {video_id} already configured \
                     — remove it first if you want to re-add"
                );
            }

            // Fetch first so we validate the id and populate the cache
            // before committing the config write. --no-fetch skips.
            let fetched_title: Option<String> = if no_fetch {
                None
            } else {
                ensure_tool("yt-dlp")?;
                eprintln!("Looking up {video_id} via yt-dlp…");
                let item = fetch_video_and_cache(&video_id)?;
                item.title
            };

            cfg.videos.push(config::ConfiguredVideo {
                video_id: video_id.clone(),
                start_offset_secs: offset.filter(|&n| n > 0),
            });
            config::save(&cfg)?;

            // Seed a position if the offset is non-zero AND there's no
            // existing position for this video (existing watch progress
            // always wins over a fresh offset hint).
            if let Some(secs) = offset
                && secs > 0
                && state::get_position(&video_id).is_none()
            {
                let pos = state::Position {
                    position_secs: secs as f64,
                    duration_secs: None,
                    updated_at: state::now_secs(),
                };
                state::upsert_position(&video_id, pos)?;
            }

            let title_suffix = fetched_title
                .as_deref()
                .map(|t| format!(" — {t}"))
                .unwrap_or_default();
            match offset {
                Some(secs) if secs > 0 => {
                    println!("added {video_id} @ {}{title_suffix}", fmt_dur(secs as f64));
                    println!("(seeded as partial; shows up in `rstube play partial`)");
                }
                _ => println!("added {video_id}{title_suffix}"),
            }
            println!("(stored in {})", config::config_path().display());
            Ok(())
        }
        VideosAction::Remove { url_or_id } => {
            let (video_id, _) = config::parse_video_spec(&url_or_id)?;
            let mut cfg = config::load();
            let Some(idx) = cfg.videos.iter().position(|v| v.video_id == video_id) else {
                bail!("no configured video with id {video_id}");
            };
            let removed = cfg.videos.remove(idx);
            config::save(&cfg)?;

            // If this video was added with an offset and its current saved
            // position still equals that offset (i.e. the user never actually
            // played it past the seed), clean up the position too. Otherwise
            // the user has real watch progress — leave it alone.
            if let Some(seeded) = removed.start_offset_secs.filter(|&n| n > 0)
                && let Some(pos) = state::get_position(&removed.video_id)
                && (pos.position_secs - seeded as f64).abs() < 0.5
            {
                if let Err(e) = state::delete_position(&removed.video_id) {
                    eprintln!("warning: failed to clear seeded position: {e}");
                }
            }
            println!("removed {video_id}");
            Ok(())
        }
        VideosAction::List => {
            let cfg = config::load();
            if cfg.videos.is_empty() {
                println!("(no videos configured)");
                println!("(config path: {})", config::config_path().display());
                return Ok(());
            }
            for (i, v) in cfg.videos.iter().enumerate() {
                let offset = v.start_offset_secs
                    .filter(|&n| n > 0)
                    .map(|n| format!(" @ {}", fmt_dur(n as f64)))
                    .unwrap_or_default();
                let title = state::find_in_playlist_cache(&v.video_id)
                    .and_then(|it| it.title)
                    .map(|t| format!("  {t}"))
                    .unwrap_or_default();
                println!("{:>2}. {}{offset}{title}", i + 1, v.video_id);
            }
            Ok(())
        }
        VideosAction::Fetch { url_or_id } => {
            let cfg = config::load();
            if cfg.videos.is_empty() {
                bail!("no videos configured — add one with `rstube videos add <url-or-id>`");
            }
            let targets: Vec<&config::ConfiguredVideo> = match url_or_id {
                Some(s) => {
                    let (video_id, _) = config::parse_video_spec(&s)?;
                    let Some(v) = cfg.videos.iter().find(|v| v.video_id == video_id) else {
                        bail!("no configured video with id {video_id}");
                    };
                    vec![v]
                }
                None => cfg.videos.iter().collect(),
            };
            ensure_tool("yt-dlp")?;
            for v in targets {
                let item = fetch_video_and_cache(&v.video_id)?;
                let dur = item.duration.map(fmt_dur).unwrap_or_else(|| "--:--".into());
                let title = item.title.as_deref().unwrap_or(&v.video_id);
                println!("{}: [{dur}] {title}", v.video_id);
            }
            Ok(())
        }
    }
}

/// Synthetic cache URL used to store a single video's metadata inside the
/// existing playlist cache. Keeps the cache format uniform.
fn video_cache_key(video_id: &str) -> String {
    format!("rstube:video:{video_id}")
}

/// Fetch a single video via yt-dlp and write to the playlist cache under the
/// synthetic key. Returns the fetched metadata.
fn fetch_video_and_cache(video_id: &str) -> Result<playlist::PlaylistItem> {
    let item = playlist::fetch_video(video_id)
        .with_context(|| format!("failed to fetch video {video_id}"))?;
    let key = video_cache_key(video_id);
    if let Err(e) = state::save_playlist_cache(&key, std::slice::from_ref(&item)) {
        eprintln!("warning: failed to save video cache: {e}");
    }
    Ok(item)
}

/// Load a single configured video's metadata — from cache if fresh, else
/// refetch via yt-dlp and refresh the cache. Respects the same 24h TTL as
/// playlists; `refresh=true` bypasses.
fn load_configured_video(video_id: &str, refresh: bool) -> Result<playlist::PlaylistItem> {
    let key = video_cache_key(video_id);
    if !refresh {
        if let Some(entry) = state::load_playlist_cache(&key) {
            if state::now_secs().saturating_sub(entry.fetched_at) < PLAYLIST_CACHE_TTL_SECS {
                if let Some(item) = entry.items.into_iter().next() {
                    return Ok(item);
                }
            }
        }
    }
    ensure_tool("yt-dlp")?;
    eprintln!("Fetching video metadata for {video_id}…");
    fetch_video_and_cache(video_id)
}

fn run_show(action: ShowAction) -> Result<()> {
    match action {
        ShowAction::Finished { details } => {
            let entries = tui::finished_candidates();
            if entries.is_empty() {
                println!("(no finished videos)");
                return Ok(());
            }
            for e in &entries {
                if details {
                    print_history_row(e);
                } else {
                    let title = e.title.as_deref().unwrap_or(&e.url);
                    println!("{title}");
                }
            }
            Ok(())
        }
        ShowAction::Partial => {
            let entries = tui::partial_candidates();
            if entries.is_empty() {
                println!("(no partial videos)");
                return Ok(());
            }
            for e in &entries {
                print_history_row(e);
            }
            Ok(())
        }
        ShowAction::New { refresh } => {
            let merged = load_merged_playlists(refresh)?;
            let seen = state::played_video_ids();
            let unseen: Vec<playlist::PlaylistItem> =
                merged.into_iter().filter(|it| !seen.contains(&it.id)).collect();
            if unseen.is_empty() {
                println!("(no new videos — every item in your playlists is in history)");
                return Ok(());
            }
            for it in &unseen {
                let title = it.title.as_deref().unwrap_or(&it.id);
                let dur = it.duration.map(fmt_dur).unwrap_or_else(|| "--:--".into());
                println!("[{dur}] {} {title}", it.id);
            }
            Ok(())
        }
    }
}

fn print_history_row(entry: &state::HistoryEntry) {
    let title = entry.title.as_deref().unwrap_or(&entry.url);
    let pos = fmt_dur(entry.position_on_exit);
    let dur = entry.duration_secs.map(fmt_dur).unwrap_or_else(|| "--:--".into());
    let pct = entry
        .duration_secs
        .filter(|d| *d > 0.0)
        .map(|d| format!(" ({:.0}%)", 100.0 * entry.position_on_exit / d))
        .unwrap_or_default();
    println!("[{pos}/{dur}{pct}] {} {title}", entry.video_id);
}

fn show_history(limit: usize, verbose: bool) -> Result<()> {
    let path = state::history_path();
    if !path.exists() {
        println!("(no history yet — path: {})", path.display());
        return Ok(());
    }
    let sessions = state::load_history_sessions();
    let start = sessions.len().saturating_sub(limit);
    for entry in &sessions[start..] {
        let title = entry.title.as_deref().unwrap_or(&entry.url);
        let unfinished = entry.ts_end == 0;
        // Unfinished session: phase-1 row with no phase-2 — mpv died before
        // writing final state. Fall back to positions.redb for the last
        // tracker-written position.
        let effective_pos = if unfinished {
            state::get_position(&entry.video_id)
                .map(|p| p.position_secs)
                .unwrap_or(entry.position_on_exit)
        } else {
            entry.position_on_exit
        };
        let effective_dur = entry.duration_secs.or_else(|| {
            state::get_position(&entry.video_id).and_then(|p| p.duration_secs)
        });
        let pos = fmt_dur(effective_pos);
        let dur = effective_dur.map(fmt_dur).unwrap_or_else(|| "--:--".into());
        let pct = effective_dur
            .filter(|d| *d > 0.0)
            .map(|d| format!(" ({:.0}%)", 100.0 * effective_pos / d))
            .unwrap_or_default();
        let marker = if unfinished { " [unclean exit]" } else { "" };
        let date_prefix = if verbose {
            let end = if entry.ts_end == 0 { "........".into() } else { fmt_ts(entry.ts_end) };
            format!("{} → {}  ", fmt_ts(entry.ts_start), end)
        } else {
            String::new()
        };
        println!("{date_prefix}[{pos}/{dur}{pct}] {} {title}{marker}", entry.video_id);
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

fn tool_present(name: &str) -> bool {
    which(name).is_ok()
}

/// Detect the system package manager. Returns the argv needed to install a
/// single package (appending the package name is the caller's job). Returns
/// None if no known manager is present.
fn detect_system_installer() -> Option<Vec<String>> {
    // Order matters: prefer distro-native over Homebrew on Linux.
    let candidates: &[(&str, &[&str])] = &[
        ("apt-get", &["sudo", "apt-get", "install", "-y"]),
        ("dnf", &["sudo", "dnf", "install", "-y"]),
        ("pacman", &["sudo", "pacman", "-S", "--noconfirm"]),
        ("zypper", &["sudo", "zypper", "install", "-y"]),
        ("brew", &["brew", "install"]),
    ];
    for (probe, argv) in candidates {
        if tool_present(probe) {
            return Some(argv.iter().map(|s| (*s).to_string()).collect());
        }
    }
    None
}

fn run_argv(argv: &[String]) -> Result<()> {
    let (head, tail) = argv.split_first().context("empty argv")?;
    let status = Command::new(head)
        .args(tail)
        .status()
        .with_context(|| format!("failed to spawn {head}"))?;
    if !status.success() {
        bail!("{} exited with {status}", argv.join(" "));
    }
    Ok(())
}

fn install_mpv() -> Result<()> {
    let Some(mut argv) = detect_system_installer() else {
        bail!(
            "could not detect a supported system package manager (apt-get, dnf, pacman, zypper, brew) — \
             install mpv manually and re-run"
        );
    };
    argv.push("mpv".to_string());
    eprintln!("Installing mpv via: {}", argv.join(" "));
    run_argv(&argv)
}

fn install_yt_dlp() -> Result<()> {
    if tool_present("pipx") {
        let argv = vec!["pipx".to_string(), "install".to_string(), "yt-dlp".to_string()];
        eprintln!("Installing yt-dlp via: {}", argv.join(" "));
        return run_argv(&argv);
    }
    if tool_present("pip") {
        let argv = vec!["pip".to_string(), "install".to_string(), "--user".to_string(), "yt-dlp".to_string()];
        eprintln!("Installing yt-dlp via: {}", argv.join(" "));
        return run_argv(&argv);
    }
    if tool_present("pip3") {
        let argv = vec!["pip3".to_string(), "install".to_string(), "--user".to_string(), "yt-dlp".to_string()];
        eprintln!("Installing yt-dlp via: {}", argv.join(" "));
        return run_argv(&argv);
    }
    bail!("neither pipx, pip, nor pip3 found — install Python+pip first, or install yt-dlp manually");
}

fn run_install_deps() -> Result<()> {
    let mut installed_any = false;

    if tool_present("mpv") {
        println!("mpv: already installed ✓");
    } else {
        install_mpv()?;
        installed_any = true;
    }

    if tool_present("yt-dlp") {
        println!("yt-dlp: already installed ✓");
    } else {
        install_yt_dlp()?;
        installed_any = true;
    }

    if !installed_any {
        println!("All dependencies already present.");
    } else {
        println!("Done. Re-run `rstube install-deps` to verify.");
    }
    Ok(())
}

fn fmt_ts(ts: u64) -> String {
    chrono::Local
        .timestamp_opt(ts as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "????-??-?? ??:??:??".into())
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
