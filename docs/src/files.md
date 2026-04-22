# File layout and environment variables

## Config

| Location | Contents |
|---|---|
| `$RSTUBE_CONFIG_DIR/config.toml` | Configured playlists (TOML) |

Fallbacks, in order:

1. `$RSTUBE_CONFIG_DIR/config.toml` if the env var is set
2. `$XDG_CONFIG_HOME/rstube/config.toml`
3. `$HOME/.config/rstube/config.toml`

## State

| Location | Contents |
|---|---|
| `$RSTUBE_STATE_DIR/positions.redb` | Per-video resume positions (redb key/value store) |
| `$RSTUBE_STATE_DIR/history.jsonl` | Playback history (JSON-lines, append-only) |
| `$RSTUBE_STATE_DIR/playlist_cache.json` | Cached playlist item lists (JSON) |

Fallbacks:

1. `$RSTUBE_STATE_DIR/...` if the env var is set
2. `$XDG_STATE_HOME/rstube/...`
3. `$HOME/.local/state/rstube/...`

## Env vars

| Var | Effect |
|---|---|
| `RSTUBE_CONFIG_DIR` | Override the config directory |
| `RSTUBE_STATE_DIR` | Override the state directory |
| `XDG_CONFIG_HOME` | Standard XDG var; respected if `RSTUBE_CONFIG_DIR` is unset |
| `XDG_STATE_HOME` | Standard XDG var; respected if `RSTUBE_STATE_DIR` is unset |

Both `RSTUBE_*` env vars are useful for testing — point rstube at a
temp directory without touching your real config or history.

## Safe to delete

Every file under these directories is regenerated on demand. If you want
a clean slate:

```bash
# nuke everything
rm -rf ~/.config/rstube ~/.local/state/rstube

# keep config, reset state
rm -rf ~/.local/state/rstube

# keep history, refresh the playlist cache
rm ~/.local/state/rstube/playlist_cache.json
```
