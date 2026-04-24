use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rstube")
}

fn run(dir: &Path, state_dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .current_dir(dir)
        .env("RSTUBE_STATE_DIR", state_dir)
        .args(args)
        .output()
        .expect("Failed to execute rstube")
}

#[test]
fn history_subcommand_reads_state_dir() {
    let dir = TempDir::new().expect("tempdir");
    let state_dir = dir.path().join("state");

    // No history yet — should succeed and mention the path.
    let out = run(dir.path(), &state_dir, &["history", "show"]);
    assert!(out.status.success(), "history failed: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("no history yet"), "expected empty-history message, got:\n{stdout}");

    // Seed a fake history line and verify it's rendered.
    std::fs::create_dir_all(&state_dir).unwrap();
    let line = serde_json::json!({
        "ts_start": 1_700_000_000_u64,
        "ts_end": 1_700_000_300_u64,
        "video_id": "dQw4w9WgXcQ",
        "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "title": "Never Gonna Give You Up",
        "duration_secs": 212.0,
        "position_on_exit": 106.0,
        "audio_only": false
    })
    .to_string();
    std::fs::write(state_dir.join("history.jsonl"), format!("{line}\n")).unwrap();

    let out = run(dir.path(), &state_dir, &["history", "show"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Never Gonna Give You Up"), "missing title:\n{stdout}");
    assert!(stdout.contains("1:46"), "expected formatted position 1:46:\n{stdout}");
    assert!(stdout.contains("50%"), "expected 50% progress:\n{stdout}");
}
