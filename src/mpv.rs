use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{self, HistoryEntry, Position};

const POLL_INTERVAL_SECS: u64 = 30;
const MIN_RESUME_SECS: f64 = 10.0;
const RESUME_TAIL_MARGIN_SECS: f64 = 10.0;

pub struct PlayRequest<'a> {
    pub url: &'a str,
    pub title: Option<&'a str>,
    pub duration_secs: Option<f64>,
    pub audio_only: bool,
}

pub fn play(req: PlayRequest<'_>) -> Result<()> {
    let key = state::url_key(req.url);
    let resume_at = compute_resume(&key, req.duration_secs);

    let sock_path: PathBuf = std::env::temp_dir().join(format!("rstube-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path);

    let mut cmd = Command::new("mpv");
    if req.audio_only {
        cmd.arg("--no-video");
    }
    cmd.arg(format!("--input-ipc-server={}", sock_path.display()));
    if let Some(secs) = resume_at {
        cmd.arg(format!("--start=+{secs}"));
        eprintln!("(resuming at {:.0}s)", secs);
    }
    cmd.arg(req.url);

    let ts_start = state::now_secs();
    let mut child = cmd.spawn().context("failed to spawn mpv")?;

    let stop = Arc::new(AtomicBool::new(false));
    let last_pos: Arc<Mutex<Option<(f64, Option<f64>)>>> = Arc::new(Mutex::new(None));

    let tracker = spawn_tracker(
        sock_path.clone(),
        key.clone(),
        stop.clone(),
        last_pos.clone(),
    );

    let status = child.wait().context("failed to wait on mpv")?;
    stop.store(true, Ordering::SeqCst);
    let _ = tracker.join();
    let _ = std::fs::remove_file(&sock_path);

    let (final_pos, final_dur) = last_pos.lock().unwrap().clone().unwrap_or((0.0, req.duration_secs));
    let duration_for_record = final_dur.or(req.duration_secs);

    if final_pos > 0.0 {
        let _ = state::upsert_position(
            &key,
            Position {
                position_secs: final_pos,
                duration_secs: duration_for_record,
                updated_at: state::now_secs(),
            },
        );
    }

    let _ = state::append_history(&HistoryEntry {
        ts_start,
        ts_end: state::now_secs(),
        video_id: key,
        url: req.url.to_owned(),
        title: req.title.map(|s| s.to_owned()),
        duration_secs: duration_for_record,
        position_on_exit: final_pos,
        audio_only: req.audio_only,
    });

    if !status.success() {
        bail!("mpv exited with status {status}");
    }
    Ok(())
}

fn compute_resume(key: &str, duration_hint: Option<f64>) -> Option<f64> {
    let pos = state::get_position(key)?;
    if pos.position_secs < MIN_RESUME_SECS {
        return None;
    }
    let dur = pos.duration_secs.or(duration_hint);
    if let Some(d) = dur
        && pos.position_secs > d - RESUME_TAIL_MARGIN_SECS
    {
        return None;
    }
    Some(pos.position_secs)
}

/// Background thread: connect to mpv's IPC socket, poll `time-pos` +
/// `duration` every POLL_INTERVAL_SECS, persist to positions.json.
fn spawn_tracker(
    sock_path: PathBuf,
    key: String,
    stop: Arc<AtomicBool>,
    last_pos: Arc<Mutex<Option<(f64, Option<f64>)>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stream = match wait_for_socket(&sock_path, &stop, Duration::from_secs(10)) {
            Some(s) => s,
            None => return,
        };
        let stream = Arc::new(stream);
        let reader_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut writer = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        };

        let reader = BufReader::new(reader_stream);
        let pending: Arc<Mutex<std::collections::HashMap<u64, String>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));
        let latest: Arc<Mutex<(Option<f64>, Option<f64>)>> = Arc::new(Mutex::new((None, None)));

        let reader_pending = pending.clone();
        let reader_latest = latest.clone();
        let reader_stop = stop.clone();
        let reader_thread = thread::spawn(move || {
            for line in reader.lines() {
                if reader_stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(line) = line else { break };
                let Ok(resp) = serde_json::from_str::<IpcResponse>(&line) else { continue };
                let Some(req_id) = resp.request_id else { continue };
                let which = {
                    let mut p = reader_pending.lock().unwrap();
                    p.remove(&req_id)
                };
                let Some(which) = which else { continue };
                if let Some(v) = resp.data.and_then(|d| d.as_f64()) {
                    let mut l = reader_latest.lock().unwrap();
                    match which.as_str() {
                        "time-pos" => l.0 = Some(v),
                        "duration" => l.1 = Some(v),
                        _ => {}
                    }
                }
            }
        });

        let mut next_id: u64 = 1;
        let mut last_persist = Instant::now()
            .checked_sub(Duration::from_secs(POLL_INTERVAL_SECS))
            .unwrap_or_else(Instant::now);

        while !stop.load(Ordering::SeqCst) {
            if last_persist.elapsed() >= Duration::from_secs(POLL_INTERVAL_SECS) {
                for prop in ["time-pos", "duration"] {
                    let id = next_id;
                    next_id += 1;
                    pending.lock().unwrap().insert(id, prop.to_owned());
                    let cmd = format!(
                        r#"{{"command":["get_property","{prop}"],"request_id":{id}}}"#
                    );
                    if writeln!(writer, "{cmd}").is_err() {
                        return;
                    }
                }
                thread::sleep(Duration::from_millis(200));

                let snapshot = latest.lock().unwrap().clone();
                if let (Some(pos), dur) = snapshot {
                    *last_pos.lock().unwrap() = Some((pos, dur));
                    let _ = state::upsert_position(
                        &key,
                        Position {
                            position_secs: pos,
                            duration_secs: dur,
                            updated_at: state::now_secs(),
                        },
                    );
                }
                last_persist = Instant::now();
            }
            thread::sleep(Duration::from_millis(500));
        }

        let _ = reader_thread.join();
    })
}

fn wait_for_socket(
    path: &std::path::Path,
    stop: &AtomicBool,
    timeout: Duration,
) -> Option<UnixStream> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if stop.load(Ordering::SeqCst) {
            return None;
        }
        if let Ok(s) = UnixStream::connect(path) {
            return Some(s);
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

#[derive(Deserialize)]
struct IpcResponse {
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    request_id: Option<u64>,
}
