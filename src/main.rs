use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "rstube")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Search and play YouTube videos via yt-dlp + mpv")]
struct Cli {
    /// Search query, or a direct YouTube URL
    query: Vec<String>,

    /// Number of search results to show
    #[arg(short = 'n', long, default_value_t = 10)]
    results: usize,

    /// Play audio only
    #[arg(short = 'a', long)]
    audio_only: bool,

    /// Auto-play the first result without prompting
    #[arg(short = 'f', long)]
    first: bool,

    /// Print full build/version info (git sha, rustc, build time) and exit
    #[arg(long = "version-full")]
    version_full: bool,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    title: String,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.version_full {
        print_version();
        return Ok(());
    }

    ensure_tool("yt-dlp")?;

    if cli.query.is_empty() {
        bail!("provide a search query or a YouTube URL");
    }
    let query = cli.query.join(" ");

    let url = if is_url(&query) {
        query
    } else {
        let entries = search(&query, cli.results)?;
        if entries.is_empty() {
            bail!("no results for {query:?}");
        }
        let choice = if cli.first { 0 } else { prompt_choice(&entries)? };
        format!("https://www.youtube.com/watch?v={}", entries[choice].id)
    };

    ensure_tool("mpv")?;
    play(&url, cli.audio_only)
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

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("www.")
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

fn search(query: &str, n: usize) -> Result<Vec<Entry>> {
    let spec = format!("ytsearch{n}:{query}");
    let output = Command::new("yt-dlp")
        .args(["-j", "--flat-playlist", "--default-search", "ytsearch", &spec])
        .output()
        .context("failed to run yt-dlp")?;
    if !output.status.success() {
        bail!(
            "yt-dlp search failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut entries = Vec::new();
    for line in output.stdout.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let entry: Entry = serde_json::from_slice(line)
            .context("failed to parse yt-dlp JSON line")?;
        entries.push(entry);
    }
    Ok(entries)
}

fn prompt_choice(entries: &[Entry]) -> Result<usize> {
    for (i, e) in entries.iter().enumerate() {
        let uploader = e.uploader.as_deref().unwrap_or("?");
        let dur = e.duration.map(fmt_dur).unwrap_or_else(|| "--:--".into());
        println!("{:>2}. [{dur}] {} — {uploader}", i + 1, e.title);
    }
    print!("Select [1-{}] (enter to pick 1, q to quit): ", entries.len());
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let trimmed = line.trim();
    if trimmed.eq_ignore_ascii_case("q") {
        std::process::exit(0);
    }
    if trimmed.is_empty() {
        return Ok(0);
    }
    let n: usize = trimmed.parse().context("invalid selection")?;
    if n == 0 || n > entries.len() {
        bail!("selection out of range");
    }
    Ok(n - 1)
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

fn play(url: &str, audio_only: bool) -> Result<()> {
    let mut cmd = Command::new("mpv");
    if audio_only {
        cmd.arg("--no-video");
    }
    cmd.arg(url);
    let status = cmd.status().context("failed to run mpv")?;
    if !status.success() {
        bail!("mpv exited with status {status}");
    }
    Ok(())
}
