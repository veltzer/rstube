# Playlist and video cache

`yt-dlp` fetches are fast but hit the network. rstube caches both
playlist item lists and single-video metadata locally, and only
refetches when the cache is more than 24 hours old.

## Where

`$XDG_STATE_HOME/rstube/playlist_cache.json` — a single JSON file keyed
by URL.

```json
{
  "entries": {
    "https://www.youtube.com/playlist?list=PL...": {
      "fetched_at": 1760000000,
      "items": [ { "id": "...", "title": "...", "duration": 1234 }, ... ]
    },
    "rstube:video:dQw4w9WgXcQ": {
      "fetched_at": 1760000000,
      "items": [ { "id": "dQw4w9WgXcQ", "title": "...", "duration": 213 } ]
    }
  }
}
```

Playlists live under their real YouTube URL. Individually-configured
videos (see [Videos](videos.md)) live under a synthetic
`rstube:video:<id>` key — reusing the same file format keeps things
simple.

## When rstube refetches

- On any `play new` / `play any` / `show new` run where the cached
  entry is ≥ 24 hours old or missing.
- On **every** run when `--refresh` is passed.
- On every `rstube playlists fetch` or `rstube videos fetch`.

Cache hits print a one-line note including the item count and age:

```
Using cached playlist for <url> (217 items, 42m old).
```

## Forcing a refresh

```bash
rstube play new --refresh
rstube playlists fetch          # refresh all playlists
rstube playlists fetch chess    # refresh one playlist
rstube videos fetch                    # refresh all single videos
rstube videos fetch dQw4w9WgXcQ        # refresh one video
```

## Pre-warming

Put the fetch commands in a cron job or systemd timer to always have
fresh data without paying the latency at `play` time:

```cron
# every 6 hours
0 */6 * * * /usr/local/bin/rstube playlists fetch >/dev/null 2>&1
0 */6 * * * /usr/local/bin/rstube videos fetch    >/dev/null 2>&1
```
