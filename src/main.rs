mod mpv;
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
    /// Open TUI picker to resume a previously played video
    Resume,
    /// Print full build/version info (git sha, rustc, build time)
    Version,
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
    }
}

fn run_resume() -> Result<()> {
    let Some(sel) = tui::run_picker()? else {
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
