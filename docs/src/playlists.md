# Playlists

rstube supports one or more configured YouTube playlists. Each is stored
under a short name you choose.

## Commands

```bash
# add
rstube playlists add <name> <url-or-id>

# remove
rstube playlists remove <name>

# show URL
rstube playlists show <name>

# list all
rstube playlists list

# refetch from YouTube (updates local cache)
rstube playlists fetch           # all configured playlists
rstube playlists fetch <name>    # just one
```

A "url-or-id" can be any of:

- A full playlist URL: `https://www.youtube.com/playlist?list=PLABCDEF`
- A URL with `list=...` and other junk: also fine
- A bare playlist id: `PLABCDEF` → normalized into a canonical URL

## Ordering matters

Playlists are stored as an ordered list. `rstube play new` scans them in
order and picks the first one with any unseen video — so put the playlist
you want to watch by default at the top.

## Visibility

For rstube to read a playlist without authentication, it must be
**public** or **unlisted** on YouTube. Private playlists are not
accessible. Unlisted is usually what you want: hidden from search and
your channel, but still fetchable by anyone with the link.

To change: YouTube Studio → Content → Playlists → pencil icon → Visibility.

## Storage

The configured list lives in a TOML config file:

- `$RSTUBE_CONFIG_DIR/config.toml` if set
- else `$XDG_CONFIG_HOME/rstube/config.toml`
- else `~/.config/rstube/config.toml`

Format:

```toml
[[playlists]]
name = "chess"
url = "https://www.youtube.com/playlist?list=PLABCDEF"

[[playlists]]
name = "tutorials"
url = "https://www.youtube.com/playlist?list=PLXYZ123"
```

You can hand-edit this file; rstube will re-read it on every invocation.
