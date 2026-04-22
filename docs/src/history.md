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

## Crash and power-loss safety

You will not lose your position if mpv crashes, your terminal is killed,
or your laptop runs out of battery mid-video.

While mpv is playing, a background thread polls the current position over
mpv's IPC socket every 30 seconds and writes it to `positions.json`. The
write is atomic (tmp-then-rename), so even a power cut mid-write cannot
corrupt the file — you end up with either the previous snapshot or the
new one, never a partial.

Worst case: you lose **up to 30 seconds** of progress — the time since
the last tick. `rstube play resume` will still show the video, and it
will start where the last tick landed.

There's one narrow exception: if playback is killed within the first 30
seconds of starting, no tick has happened yet, so nothing new is saved.
Any *previous* saved position for that video is left untouched — you'd
resume from there, not from zero.

Unclean-exit sessions (mpv killed by crash or power loss) **do** appear
in `rstube history` — flagged with a `[unclean exit]` marker. See
"Two-phase history writes" below for how.

## How the tracker stays in sync with mpv

rstube does not drive mpv's playback. mpv owns the playhead, and rstube
just snapshots it.

When `rstube` spawns mpv it passes `--input-ipc-server=<socket>`, which
makes mpv expose a JSON-over-unix-socket interface. A background thread
in rstube connects to that socket and, once every 30 seconds, asks mpv
for two properties:

- `time-pos` — the current playhead, in seconds
- `duration` — the video's total length, in seconds (if known)

Whatever mpv reports is written to `positions.json` as-is. The tracker
doesn't track a playing/paused/buffering state of its own — it just
snapshots whatever mpv says the playhead is. That means:

- **Paused:** ticks keep firing, but `time-pos` doesn't move, so the
  saved position just keeps getting overwritten with the same value.
- **Scrubbed:** the next tick picks up the new position and saves it.
- **Buffering / not yet loaded:** mpv returns `null` for `time-pos`, and
  the tracker silently skips that tick — no overwrite happens.

When mpv exits normally, rstube takes one final snapshot and writes the
last known position and the full history entry. When mpv dies
uncleanly, the last periodic snapshot is what survives.

## Where positions are saved

A single local file, resolved in this order:

1. `$RSTUBE_STATE_DIR/positions.json` if that env var is set
2. `$XDG_STATE_HOME/rstube/positions.json`
3. `$HOME/.local/state/rstube/positions.json`

On most Linux systems that's `~/.local/state/rstube/positions.json`.

Format — a flat JSON object keyed by YouTube video id:

```json
{
  "dQw4w9WgXcQ": {
    "position_secs": 127.3,
    "duration_secs": 213.0,
    "updated_at": 1713789012
  },
  "another-id": { "...": "..." }
}
```

- `position_secs` — where the playhead was at the last tick (seconds).
- `duration_secs` — the total length if mpv has reported it yet, else
  null. Used by the resume picker to hide near-finished videos.
- `updated_at` — unix timestamp of the last tick. Not used for anything
  user-visible; useful when debugging.

The file is rewritten in full on every update, via a tmp-then-rename.
That's atomic with respect to power loss: you end up with either the
old version or the new one, never a torn write.

## Two-phase history writes

`history.jsonl` uses a two-phase append scheme so that even an unclean
exit (SIGKILL, OOM kill, power loss) still leaves a record of the
session.

**Phase 1 — at spawn.** Before calling `child.wait()`, rstube appends an
open-session line: `ts_start` is the current time, `ts_end` is `0`, and
`position_on_exit` is `0.0`. The rest of the fields (video id, URL,
title, audio-only flag) are all known at spawn time. This write happens
in the first few milliseconds of a session.

**Phase 2 — at clean exit.** When mpv exits cleanly, rstube appends a
second line with the same `(video_id, ts_start)` key, this time with a
real `ts_end` and the final `position_on_exit` pulled from the last
tracker snapshot.

**Read-side.** `load_history_sessions()` in `state.rs` groups lines by
`(video_id, ts_start)` and keeps the line with the greater `ts_end`.
Phase-2 always replaces phase-1 for a clean session; phase-1-only
sessions (rstube/mpv died before reaching phase-2) survive as-is.

**User-visible effect.** `rstube history` shows unclean-exit sessions
with a `[unclean exit]` suffix, and pulls the last-known position from
`positions.json` so you can still see roughly where you were.

### Why not a SIGINT / SIGTERM handler?

A signal handler would help with Ctrl-C or `SIGTERM` from your terminal
closing, but it is the wrong tool for the job:

- **SIGKILL and SIGSTOP are unmaskable.** Kernel kills (OOM), power
  loss, and `kill -9` bypass any handler.
- **Power loss gives you zero cycles.** No signal at all fires.
- Real signal handlers are restricted to async-signal-safe calls — you
  cannot call `serde_json` or `fs::write` from them, which means you'd
  need a pipe-based wake-up pattern and a reserved writer thread.
  Significant complexity for partial coverage.

The two-phase approach catches all three failure modes (SIGTERM, hard
kills, power loss) with no concurrency new code and no signal handling.

### Why not `fsync(2)` after each write?

Skipped intentionally.

A `write(2)` to an `O_APPEND` file puts the data in the kernel's page
cache. That data survives the rstube process dying for any reason
(SIGKILL, OOM kill, panic) — the kernel keeps flushing it out on its
own schedule (every 5–30s on Linux, driven by `vm.dirty_*`). `fsync`
only protects the narrow window of a **kernel crash or sudden power
loss** between `write` returning and the kernel's flush.

For rstube specifically:

- We already tolerate ~30 seconds of position loss from the tracker's
  30s poll interval.
- A lost phase-1 line for a session whose `positions.json` entry did
  flush means the session is missing from `rstube history` — but `play
  resume` still works, because resume reads from `positions.json`.
- `fsync` serializes IO, costing 5–50ms per write on a spinning disk;
  cheap on SSD but not free.

The durability we actually want — phase-1 surviving an rstube crash —
is delivered by plain `write(2)`. Adding `fsync` would only close the
"power cut in the 30-second flush window" gap, which is less impactful
than the gap we just closed (unclean exit leaves no record at all).

## Storage — all state files

| File | Contents |
|---|---|
| `positions.json` | Resume positions (see above) |
| `history.jsonl` | One JSON line per playback session, append-only |
| `playlist_cache.json` | Cached playlist item lists, keyed by URL |

All three live under the same state directory (`RSTUBE_STATE_DIR` →
`XDG_STATE_HOME/rstube` → `~/.local/state/rstube`). See [File layout and
environment](files.md) for the full env-var story.

Safe to delete any of these — rstube recreates them on demand. Deleting
`positions.json` just means `play resume` will be empty until you watch
something; deleting `history.jsonl` clears the log.

## Syncing across machines

rstube doesn't sync anywhere. If you want the same positions on another
machine, sync the state directory yourself — `syncthing` pointed at
`~/.local/state/rstube`, or a periodic `rsync`, or just `scp
~/.local/state/rstube/positions.json` before switching machines.

Nothing rstube writes is tied to a machine or user account; the file is
portable.
