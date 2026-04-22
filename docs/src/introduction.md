# rstube

A terminal app for playing YouTube videos through `mpv`, with per-video
position tracking and playlist-aware pickers.

## What it does

- Reads one or more YouTube playlists you've configured.
- Shows a TUI picker to pick something to watch — filtered by what you
  haven't seen yet, or unfiltered, or only videos you were partway through.
- Launches `mpv` with the right resume position.
- Records where you left off so you can resume next time.

## What it doesn't do

- Download videos. It streams through mpv via `yt-dlp`.
- Talk to the YouTube API for authenticated data. Your YouTube account's
  per-video watch progress isn't accessible; rstube tracks positions
  locally from mpv sessions only.
- Manage mpv itself. Keybindings, output config, subs — that lives in your
  `~/.config/mpv/`. See [mpv configuration](mpv-config.md).

## How it works

1. `yt-dlp --flat-playlist` fetches a playlist's video list in ~1s. Cached
   to `$XDG_STATE_HOME/rstube/playlist_cache.json` for 24 hours.
2. The TUI picker (ratatui + crossterm) shows the filtered list.
3. On Enter, rstube spawns `mpv <url> --start=<resume-secs>` and streams
   mpv's IPC to write the current position to
   `$XDG_STATE_HOME/rstube/positions.redb`.
4. On mpv exit, a line is appended to `history.jsonl`.

## Requirements

- `mpv` (system package)
- `yt-dlp` (Python package)

Run `rstube install-deps` to install whichever is missing.
