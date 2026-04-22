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

Note that `history.jsonl` is only appended on a clean mpv exit, so a
session that ended via crash or power loss will not appear in `rstube
history`. The saved position is still there, so `play resume` works.

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
