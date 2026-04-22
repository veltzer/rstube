# Commands

Top-level commands in `rstube`.

| Command | Description |
|---|---|
| `rstube play resume [-v]` | Pick an in-progress video from history |
| `rstube play new [--refresh] [--pick] [-v]` | Pick an unwatched video from a configured playlist |
| `rstube play any [--refresh] [-v]` | Pick any video across all configured playlists |
| `rstube history [-n N]` | Show the last N history entries (default 20) |
| `rstube playlists ...` | Manage configured playlists (see [Playlists](playlists.md)) |
| `rstube install-deps` | Install missing `mpv` / `yt-dlp` |
| `rstube complete <shell>` | Print shell completion script |
| `rstube version` | Print build info (git sha, rustc, build time) |

## `play` subcommands

### `play resume`

Opens the resume picker with in-progress videos — defined as any history
entry where the saved position is ≥ 10 seconds and more than 10 seconds
before the end. If there are no such entries, prints a clear message and
exits.

### `play new`

- Scans configured playlists in order; opens the picker on the first one
  with any unseen video.
- `--pick` — open a playlist chooser TUI first and use whichever you
  select.
- `--refresh` — bypass the 24h cache and refetch the playlist(s) being
  used.
- If *every* configured playlist has zero unseen items, prints a message
  and exits.

### `play any`

- Merges every video from every configured playlist into one picker
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
