# Videos

Alongside playlists, rstube supports adding **individual videos** under short
names. A configured video is just a 1-item "source" that feeds into the same
pickers and filters as playlists.

## When to use videos vs playlists

- **Playlist** — you want every video from a YouTube playlist URL, and the
  list grows/shrinks as the creator updates it.
- **Video** — you want a specific single video to stay in your rotation
  regardless of which playlist (if any) it came from. Handy for one-off
  recommendations a friend sent you, or a video you plan to rewatch.

Both types coexist. A video id that appears in both a configured playlist
and a configured videos entry is deduped on merge — you'll only see it once.

## Commands

```bash
# add
rstube videos add <name> <url-or-id>

# remove
rstube videos remove <name>

# show id
rstube videos show <name>

# list all
rstube videos list

# refetch title/duration from YouTube (updates local cache)
rstube videos fetch           # all configured videos
rstube videos fetch <name>    # just one
```

A "url-or-id" can be any of:

- Full watch URL: `https://www.youtube.com/watch?v=dQw4w9WgXcQ`
- Watch URL with tracking params: `https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10s` (trailing params are stripped)
- Short URL: `https://youtu.be/dQw4w9WgXcQ`
- Bare 11-char id: `dQw4w9WgXcQ`

rstube normalizes all of these to the bare id and stores only the id in
config.

## How configured videos flow through commands

Configured videos are appended to the merged pool after playlists:

- `rstube play new` — one picker of every unseen video across all playlists
  and individually-configured videos.
- `rstube play any` — one picker of every video across the same sources,
  history-filtering disabled.
- `rstube show new` — text-only equivalent of `play new`.

If a configured video has already been played (appears in history), it
counts as "seen" and is filtered out of `play new` / `show new`. Use
`play any` or the resume/partial flow to get to it again.

## Metadata fetching

Adding a video is cheap — `videos add` only stores the id. Titles and
durations are resolved lazily the first time rstube needs them (e.g. when
you run `play new`, `play any`, or `show new`).

Lookup uses `yt-dlp -j --no-playlist <url>`, which is a full per-video
call (1-3s each). Results are cached in `playlist_cache.json` under a
synthetic key `rstube:video:<id>` for 24 hours, same TTL as the playlist
cache. Force a refresh with:

```bash
rstube videos fetch
# or
rstube play new --refresh
```

## Storage

Stored in your TOML config alongside playlists:

```toml
[[playlists]]
name = "chess"
url = "https://www.youtube.com/playlist?list=PLABCDEF"

[[videos]]
name = "rick"
video_id = "dQw4w9WgXcQ"

[[videos]]
name = "that-talk"
video_id = "kxopViU98Xo"
```

Same config file as playlists — see [Playlists](playlists.md) for the
location rules.
