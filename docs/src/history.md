# Watch history and resume

rstube maintains two pieces of per-video state:

- **positions** — where you left off on a given video id. Updated live as
  mpv plays, so killing the terminal doesn't lose progress.
- **history** — a JSON-lines log of every playback session. Append-only.

## What counts as "resumable"

`rstube play resume` shows history entries where:

- The saved position is ≥ 10 seconds (quick-quit noise is ignored).
- If the duration is known, the position is more than 10 seconds before
  the end (finished videos don't show up again).

If you open a video and immediately quit (before 10 seconds), the new
0-second entry does **not** shadow an older session that was actually
watched — the resume picker prefers the most recent *resumable* session
per video, not the most recent session overall.

## Commands that interact with history

- `rstube history [-n N]` — print the last N entries.
- `rstube play resume` — pick an in-progress video.
- Inside any picker, `d` removes the selected entry's saved position —
  useful if a video gets stuck in a partially-watched state you don't
  want to resume.

## Storage

- `$RSTUBE_STATE_DIR/positions.json` (or `$XDG_STATE_HOME/rstube/...`)
  — JSON map: video id → `{position_secs, duration_secs, updated_at}`.
- `$RSTUBE_STATE_DIR/history.jsonl` — one JSON object per line, appended
  per session.

These files are atomic writes (tmp-then-rename) for positions.json, and
plain appends for history.jsonl. Safe to delete if you want to start
over; rstube recreates them on the next run.
