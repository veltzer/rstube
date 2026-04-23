# Videos

Alongside playlists, rstube supports adding **individual videos** by id.
A configured video is just a 1-item "source" that feeds into the same
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
# add (looks up title + validates the id via yt-dlp by default)
rstube videos add <url-or-id>
rstube videos add <url-or-id> --start 1:23    # override/set start offset
rstube videos add <url-or-id> --no-fetch      # skip the yt-dlp lookup

# remove (accepts any URL shape or the bare id)
rstube videos remove <url-or-id>

# list all (shows cached titles if fetched)
rstube videos list

# refetch title/duration from YouTube (updates local cache)
rstube videos fetch              # all configured videos
rstube videos fetch <url-or-id>  # just one
```

A "url-or-id" can be any of:

- Full watch URL: `https://www.youtube.com/watch?v=dQw4w9WgXcQ`
- Watch URL with `&t=...`: `https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10s`
- Short URL: `https://youtu.be/dQw4w9WgXcQ`
- Short URL with `?t=...`: `https://youtu.be/gl2GaCDt8BE?t=178`
- Bare 11-char id: `dQw4w9WgXcQ`

All shapes normalize to the same 11-char id, which is both the config
identity and how you refer to a configured video later.

## No duplicates

rstube rejects a `videos add` if the **video id** is already configured,
regardless of URL shape. Error:

```
Error: video id dQw4w9WgXcQ already configured —
       remove it first if you want to re-add
```

To re-add (e.g. to change the offset), remove first:

```bash
rstube videos remove dQw4w9WgXcQ
rstube videos add "https://youtu.be/dQw4w9WgXcQ?t=90"
```

## Start offsets

If the URL includes a `t=...` query parameter, rstube extracts the
offset and treats the video as **already in progress** — it will show
up in `rstube play partial` / `show partial` immediately, and when you
play it mpv starts at that offset instead of zero.

Offset values accept seconds (`178`, `178s`), YouTube's compound form
(`2m58s`, `1h2m3s`), and colon form (`2:58`, `1:02:03`).

You can also set an offset explicitly:

```bash
rstube videos add "https://youtu.be/abc..." --start 1m23s
rstube videos add "https://youtu.be/abc..." --start 1:23
rstube videos add "https://youtu.be/abc..." --start 83
```

If both `--start` and a URL `t=` are present, `--start` wins.

### How to copy a timestamped URL from YouTube

1. **From the web UI:** pause the video at the moment you want; right-click
   on the video itself → "Copy video URL at current time".
2. **From the Share button:** click Share, tick "Start at [current time]",
   copy the URL shown.
3. **From the mobile app:** Share sheet has a "Start at" toggle.

### What happens at play time

- If there's no saved position yet, playback starts at the configured
  offset.
- If there's already a saved position (because you've played the video
  past the offset), the saved position wins — rstube's own progress
  always beats a fresh-start hint.
- Removing a configured video clears the seeded position only if you
  never played it past the offset. Real watch progress is preserved (use
  `rstube forget partial` to wipe that too).

## How configured videos flow through commands

Configured videos are appended to the merged pool after playlists:

- `rstube play new` — one picker of every unseen video across all playlists
  and individually-configured videos.
- `rstube play any` — one picker of every video across the same sources,
  history-filtering disabled.
- `rstube show new` — text-only equivalent of `play new`.

If a configured video has already been played (appears in history), it
counts as "seen" and is filtered out of `play new` / `show new`. Use
`play any` or the partial/finished flow to get to it again.

## Metadata fetching

By default `videos add` calls `yt-dlp -j --no-playlist <url>` to
validate the id and fetch the title and duration. This takes 1–3
seconds per add; if the id is invalid (malformed, removed, region-
locked for your network) `videos add` aborts and nothing is written
to config. On success the title shows up in the success line, and
`videos list` prints it alongside the id afterwards.

Pass `--no-fetch` to skip the lookup — useful for bulk-importing from
a script, or for private/unlisted videos that yt-dlp can't reach
without cookies.

```bash
rstube videos add "https://youtu.be/abc..."            # fetches + validates
rstube videos add "https://youtu.be/abc..." --no-fetch # skips, adds unconditionally
```

Results are cached in `playlist_cache.json` under a synthetic key
`rstube:video:<id>` for 24 hours, same TTL as the playlist cache.
Force a refresh with:

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
video_id = "dQw4w9WgXcQ"

# Video with a start offset — added via `videos add ... ?t=178`
# or `--start 2:58`. Shows up as partial from 2:58.
[[videos]]
video_id = "kxopViU98Xo"
start_offset_secs = 178
```

Playlists still use names because YouTube playlist ids are ugly
34-char strings; videos don't need them because the 11-char video id
is memorable enough and unambiguous.

Same config file as playlists — see [Playlists](playlists.md) for the
location rules.

## Behavior summary

A few things about configured videos that are worth calling out
explicitly, because they differ from playlists:

- **No names.** A configured video is identified solely by its
  11-char YouTube id. There is no `videos show` — `videos list`
  prints everything. `remove` and `fetch` accept any URL shape (not
  just the bare id), which makes paste-from-browser work naturally.
- **Duplicates are rejected.** `videos add` bails if the resolved id
  is already in the config. Different URL shapes of the same video
  count as the same video.
- **Adding validates.** By default `videos add` calls yt-dlp to
  confirm the id is real and to populate the title/duration cache.
  Pass `--no-fetch` to skip that.
- **`?t=N` means "seed as partial."** A URL timestamp doesn't just
  set mpv's start flag; it also writes a position record into
  `positions.redb`, so the video is classified as partial from the
  moment you add it. `--start` does the same. Remove clears the
  seeded position only if you never played past it — real watch
  progress is preserved.
- **Merge semantics.** Configured videos are appended after
  playlists in the merged pool for `play new` / `play any` /
  `show new`. A video id that appears both in a configured playlist
  and in the videos list shows up once (first occurrence wins;
  playlists come first).

## Shell completion

Bash completion for `videos remove <TAB>` and `videos fetch <TAB>`
suggests the configured video ids (read from `config.toml` at tab
time). See [Shell completion](completion.md) for install details.
