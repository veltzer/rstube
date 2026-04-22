# Playlist cache

`yt-dlp --flat-playlist` is fast (~1s for hundreds of videos), but it
still hits the network. rstube caches every fetched playlist locally and
only refetches when the cache is more than 24 hours old.

## Where

`$XDG_STATE_HOME/rstube/playlist_cache.json` — a single JSON file keyed by
playlist URL.

```json
{
  "entries": {
    "https://www.youtube.com/playlist?list=PL...": {
      "fetched_at": 1760000000,
      "items": [ { "id": "...", "title": "...", "duration": 1234 }, ... ]
    }
  }
}
```

## When rstube refetches

- On any `play new` or `play any` run where the cached entry for that
  playlist URL is ≥ 24 hours old, or missing.
- On **every** run when `--refresh` is passed.
- On every `rstube playlists fetch`.

Cache hits print a one-line note including the item count and age:

```
Using cached playlist for <url> (217 items, 42m old).
```

## Forcing a refresh

```bash
rstube play new --refresh
rstube playlists fetch          # refresh all
rstube playlists fetch chess    # refresh one
```

## Pre-warming

Put `rstube playlists fetch` in a cron job or systemd timer to always
have fresh data without paying the latency at `play` time.

```cron
# every 6 hours
0 */6 * * * /usr/local/bin/rstube playlists fetch >/dev/null 2>&1
```
