# Commands

Top-level commands in `rstube`.

| Command | Description |
|---|---|
| `rstube play partial [-v]` | Pick a partially-watched video from history |
| `rstube play new [--refresh] [--pick] [-v]` | Pick an unwatched video from a configured playlist or video |
| `rstube play any [--refresh] [-v]` | Pick any video across all configured playlists and videos |
| `rstube show finished [-d]` | Print videos watched to the end (`-d` adds timing/percentage) |
| `rstube show partial` | Print videos partially watched (same set as `play partial`) |
| `rstube show new [--refresh]` | Print videos in your playlists/videos list that you haven't started |
| `rstube history [-n N]` | Show the last N history entries (default 20) |
| `rstube playlists ...` | Manage configured playlists (see [Playlists](playlists.md)) |
| `rstube videos ...` | Manage individually configured videos (see [Videos](videos.md)) |
| `rstube install-deps` | Install missing `mpv` / `yt-dlp` |
| `rstube complete <shell>` | Print shell completion script |
| `rstube version` | Print build info (git sha, rustc, build time) |

## `play` subcommands

### `play partial`

Opens the partial picker with partially-watched videos — defined as any
history entry where the saved position is ≥ 10 seconds and more than 30
seconds before the end. If there are no such entries, prints a clear
message and exits.

### `play new`

- Scans configured playlists in order, then the videos list; opens the
  picker on the first source with any unseen video.
- `--pick` — open a chooser TUI first listing every playlist plus a
  final "videos" bucket for individually-configured videos. Select the
  one you want.
- `--refresh` — bypass the 24h cache and refetch whichever source is
  being used.
- If every playlist and the videos list have zero unseen items, prints a
  message and exits.

### `play any`

- Merges every video from every configured playlist and every configured video into one picker
  (deduplicated by video id).
- Does *not* filter by watch history — useful for rewatching.
- `--refresh` — bypass the cache.

### `-v` / `--verbose` (all three `play` subcommands)

By default rstube silences mpv's stdout — no status line, no chatter.
mpv's stderr stays connected so real errors (yt-dlp failures, network
issues) are still visible.

Pass `-v` to reconnect stdout and add `--msg-level=all=v` to the mpv
command, giving you the full status line and verbose log. Use it when
you're debugging a stream that isn't working.

## `show` subcommands

Text-only siblings of the `play` pickers — same data, no TUI, pipeable.

### `show finished`

Prints every video you've watched to within 30 seconds of the end. One
line per video, deduped by video id (most recent session kept), most
recent first. Default format is just the title:

```
<title>
```

Pass `-d` / `--details` to include the video id, timing, and percentage:

```
[pos/dur (pct%)] <id> <title>
```

### `show partial`

Same set of videos `play partial` would show — partially watched (≥10s
in, >30s before the end).

### `show new`

Every video across all configured playlists and individually configured
videos that isn't yet in your history. Uses the 24h cache by default;
pass `--refresh` to refetch. Format:

```
[duration] title
```

Note: `new` counts any history entry as "seen", including phase-1
open-session lines. If you *started* a video (even briefly), `show new`
will not list it again.

## Pickers — keys

- `↑`/`↓` or `j`/`k` — move
- `PgUp`/`PgDn` — jump by 10
- `Home`/`End` — first / last row
- `/` — focus the filter input; `Esc` or `Enter` returns focus to the list
- `a` — toggle audio-only playback for the selected item
- `d` — delete the selected row from history (and its saved position)
- `Enter` — play the selected item
- `q` or `Esc` — quit without playing

## `history`

Prints a compact one-line-per-entry view of recent plays:

```
[12:34/45:00 (27%)] Video title
```

`-n` controls the count (default 20, most recent first).
