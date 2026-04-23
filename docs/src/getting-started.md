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
rstube play partial
```

Shows videos in your watch history where you're partway through (at least
10 seconds in, and not within 30 seconds of the end). Enter resumes at
your last position.

## 4. Browse anything

```bash
rstube play any
```

Every video from every configured playlist in one picker — no history
filtering. Useful for rewatching, or when you can't remember which
playlist a video was in.

## 5. See what's where, as text

`show` mirrors `play` but prints to stdout instead of opening a picker —
pipeable to `grep`, `wc`, anything:

```bash
rstube show new          # every unwatched video
rstube show partial      # every partially-watched video
rstube show finished     # titles of finished videos (add -d for details)
```

## 6. Forget a video

If you want a video to re-appear as "new" (e.g. to rewatch from
scratch):

```bash
rstube forget partial    # picker over partial videos
rstube forget finished   # picker over finished videos
```

Select with Enter; the video's history lines and saved position are
removed. It'll show up in `play new` again next time.

## 7. Useful extras

- `--refresh` on `play new` or `play any` bypasses the 24h cache and
  refetches from YouTube.
- `rstube playlists fetch` pre-warms the cache for all configured
  playlists. Nice as a cron/systemd-timer job.
- `rstube videos add <url-or-id>` bookmarks a single video so it shows
  up in `play new`, `play any`, and `show new` alongside your playlists.
  Passing a URL with `?t=178` (or `--start 2:58`) seeds it as a partial
  starting at that offset. See [Videos](videos.md).
- In any picker: `/` to filter, `a` to toggle audio-only playback, `d` to
  delete the current row from history, `q` to quit.
- Pass `-v` / `--verbose` to any `play` subcommand to see mpv's terminal
  status line and verbose log — useful for debugging a stream that isn't
  playing.
