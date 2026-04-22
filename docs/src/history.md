# Watch history and resume

rstube maintains two pieces of per-video state:

- **positions** — where you left off on a given video id. Updated live as
  mpv plays, so killing the terminal doesn't lose progress.
- **history** — a JSON-lines log of every playback session. Append-only.

## What counts as "partial" vs "finished"

Classification is based on each video's **most recent meaningful session**
— "meaningful" meaning position ≥ 10 seconds, so an accidental
reopen-and-immediately-quit can't shadow a real session.

A session is:

- **Partial** — position ≥ 10s, and more than 30s before the end (or
  duration is unknown).
- **Finished** — duration is known and position is within 30s of the end.
- **Trivial** — less than 10s watched. Ignored for classification.

Each video appears in **exactly one** bucket at a time, based on its
most recent non-trivial session. If you finish a video and then start
watching it again, the new partial session immediately moves it out of
the finished bucket and into partial.

`rstube show partial` / `play partial` read from the partial bucket;
`show finished` reads from the finished bucket.

## Commands that interact with history

- `rstube history [-n N]` — print the last N entries. Unclean-exit
  sessions appear with a `[unclean exit]` marker.
- `rstube play partial` — pick an in-progress video to resume.
- `rstube show partial` / `show finished` — print text-only lists of
  videos classified from history + positions.
- `rstube forget partial` / `forget finished` — pick a video to forget.
  Removes **all** of its history lines (phase-1 and phase-2) plus the
  entry in `positions.redb`, so the video reappears as "new". The
  rewrite of `history.jsonl` is atomic (tmp-then-rename).
- Inside any picker, `d` removes only the selected entry's saved
  position (leaves history intact). Lighter-weight than `forget` —
  useful if you want the video to stop resuming but still count as
  "seen".

## Crash and power-loss safety

You will not lose your position if mpv crashes, your terminal is killed,
or your laptop runs out of battery mid-video.

While mpv is playing, a background thread polls the current position over
mpv's IPC socket every 30 seconds and upserts it into `positions.redb`.
redb commits use MVCC + fsync, so even a power cut mid-write cannot
corrupt the file — the commit either lands fully or not at all.

Worst case: you lose **up to 30 seconds** of progress — the time since
the last tick. `rstube play partial` will still show the video, and it
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

Whatever mpv reports is compared bit-for-bit against the last value
written to `positions.redb`. If the `(time-pos, duration)` pair is
unchanged, the tick is a no-op — no write transaction fires. Otherwise
the new value is upserted. The tracker doesn't model
playing/paused/buffering state itself; the dedup naturally falls out of
"did the playhead actually move?". That means:

- **Paused:** ticks keep firing, but `time-pos` doesn't move, so the
  bit-fingerprint matches and no redb write happens. A multi-hour pause
  costs zero writes.
- **Scrubbed:** the next tick picks up the new position, the
  fingerprint differs, a single write fires.
- **Buffering / not yet loaded:** mpv returns `null` for `time-pos`, and
  the tracker silently skips that tick — no write happens.

The comparison uses `f64::to_bits` equality, not a floating-point
tolerance. We only skip if mpv reported the identical float — not an
"approximately same" value — which sidesteps any question of what
counts as a meaningful move. During normal playback `time-pos` advances
by roughly the tick interval between samples, so bit equality never
false-positive skips a real update.

When mpv exits normally, rstube takes one final snapshot and writes the
last known position and the full history entry. When mpv dies
uncleanly, the last periodic snapshot is what survives.

## Where positions are saved

A single local file, resolved in this order:

1. `$RSTUBE_STATE_DIR/positions.redb` if that env var is set
2. `$XDG_STATE_HOME/rstube/positions.redb`
3. `$HOME/.local/state/rstube/positions.redb`

On most Linux systems that's `~/.local/state/rstube/positions.redb`.

Storage is a [redb](https://www.redb.org) key/value database. Keys are
YouTube video ids (strings); values are JSON-serialized records with
these fields:

- `position_secs` — where the playhead was at the last tick (seconds).
- `duration_secs` — the total length if mpv has reported it yet, else
  null. Used by the partial picker to hide near-finished videos.
- `updated_at` — unix timestamp of the last tick. Not used for anything
  user-visible; useful when debugging.

Each tick performs a single per-key update inside a redb write
transaction. redb uses MVCC + copy-on-write B-trees with an fsync'd
commit, so a write either lands fully or not at all; a crash or power
cut never leaves the file in a torn state.

The file isn't human-readable, so `cat` won't work. To inspect, use
`redb dump` (from the `redb-cli` crate) or any small Rust program that
opens the same table definition.

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
`positions.redb` so you can still see roughly where you were.

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
- A lost phase-1 line for a session whose `positions.redb` entry did
  flush means the session is missing from `rstube history` — but `play
  resume` still works, because resume reads from `positions.redb`.
- `fsync` serializes IO, costing 5–50ms per write on a spinning disk;
  cheap on SSD but not free.

The durability we actually want — phase-1 surviving an rstube crash —
is delivered by plain `write(2)`. Adding `fsync` would only close the
"power cut in the 30-second flush window" gap, which is less impactful
than the gap we just closed (unclean exit leaves no record at all).

## Storage — all state files

| File | Contents |
|---|---|
| `positions.redb` | Resume positions (see above) |
| `history.jsonl` | One JSON line per playback session; `forget` rewrites to remove matching lines |
| `playlist_cache.json` | Cached playlist item lists, keyed by URL |

All three live under the same state directory (`RSTUBE_STATE_DIR` →
`XDG_STATE_HOME/rstube` → `~/.local/state/rstube`). See [File layout and
environment](files.md) for the full env-var story.

Safe to delete any of these — rstube recreates them on demand. Deleting
`positions.redb` just means `play partial` will be empty until you watch
something; deleting `history.jsonl` clears the log.

## Syncing across machines

rstube doesn't sync anywhere. If you want the same positions on another
machine, sync the state directory yourself — `syncthing` pointed at
`~/.local/state/rstube`, or a periodic `rsync`, or just `scp
~/.local/state/rstube/positions.redb` before switching machines.

Nothing rstube writes is tied to a machine or user account; the file is
portable.
