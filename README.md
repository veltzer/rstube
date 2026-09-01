# rstube

Search and play YouTube videos from the terminal.

rstube reads YouTube playlists and individually configured videos, offers a
TUI picker filtered by what you have not watched yet, launches `mpv` at the
right resume position and records where you left off. It streams via
`yt-dlp` and never downloads or talks to the YouTube API.

Full documentation lives in the `docs/` mdBook and is published at
<https://veltzer.github.io/rstube>.

## Quick start

```bash
cargo install --path .
rstube install-deps          # mpv and yt-dlp
rstube playlists add chess "https://www.youtube.com/playlist?list=..."
rstube play new
```
