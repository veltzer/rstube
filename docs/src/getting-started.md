# Getting Started

A five-minute walkthrough.

## 1. Add a playlist

Grab a YouTube playlist URL (must be public or unlisted — rstube doesn't
do authenticated fetches) and add it under a short name:

```bash
rstube playlists add chess "https://www.youtube.com/playlist?list=PLABCDEF..."
```

You can pass a bare playlist id instead of a full URL:

```bash
rstube playlists add chess PLABCDEF...
```

List what you've got:

```bash
rstube playlists list
```

## 2. Play something new

```bash
rstube play new
```

This fetches the playlist (first run — cached afterwards), filters out
videos you've already watched, and opens a TUI picker. Pick one with
Enter, and mpv launches.

If every configured playlist has nothing unseen, you'll get a clear
message rather than a silent no-op.

## 3. Resume a video you started

```bash
rstube play resume
```

Shows videos in your watch history where you're partway through (at least
10 seconds in, and not within 10 seconds of the end). Enter resumes at
your last position.

## 4. Browse anything

```bash
rstube play any
```

Every video from every configured playlist in one picker — no history
filtering. Useful for rewatching, or when you can't remember which
playlist a video was in.

## 5. Useful extras

- `--pick` on `play new` opens a playlist chooser first, so you can
  override the default "first playlist with anything unseen" behaviour.
- `--refresh` on `play new` or `play any` bypasses the 24h cache and
  refetches from YouTube.
- `rstube playlists fetch` pre-warms the cache for all configured
  playlists. Nice as a cron/systemd-timer job.
- In any picker: `/` to filter, `a` to toggle audio-only playback, `d` to
  delete the current row from history, `q` to quit.
