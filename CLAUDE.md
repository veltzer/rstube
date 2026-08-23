# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Terminal app for playing YouTube videos through `mpv` with resume-position tracking. Fetches playlists via `yt-dlp --flat-playlist` (cached 24h), presents a ratatui TUI picker, launches mpv with an IPC socket to track playback position, and persists state in a redb database plus a `history.jsonl` log.

## Commands

- Build/test/lint: standard `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`.
- **CI runs no tests or lints** (workflows only build releases on tags and deploy docs). Run `cargo test` and `cargo clippy` locally before committing — there is no safety net.
- Docs are mdBook: `mdbook build docs` (deployed to GitHub Pages on push to main). `docs/book/` is gitignored build output.
- Profiling build with debug symbols: `cargo build --profile profiling` (binary in `target/profiling/`).

## Testing

- Integration tests only, in `tests/` — they run the compiled binary via `env!("CARGO_BIN_EXE_rstube")` (see `tests/common/mod.rs`).
- Tests must isolate state: set `RSTUBE_STATE_DIR` (and `RSTUBE_CONFIG_DIR` if config is touched) to a `tempfile` dir so tests never touch the real `$XDG_STATE_HOME/rstube`. Follow the pattern in `tests/tests_mod/persistence.rs`.

## Architecture notes

- Single binary crate; CLI subcommand logic lives in `src/main.rs`, with modules: `config` (TOML config + XDG paths), `state` (redb positions + JSONL history), `mpv` (IPC socket, position polling), `tui` (ratatui picker), `playlist`.
- Path resolution order everywhere: `RSTUBE_*_DIR` env var → `XDG_*_HOME` → `~/.config` / `~/.local/state`.
- `build.rs` bakes git SHA/branch/dirty-state and rustc version into the binary for `rstube version` — it reruns on `.git/HEAD` changes, so version output reflects the working tree at build time.
- Error handling: `anyhow::Result` with `.context(...)` throughout; no custom error types, no unsafe code.

## Runtime dependencies

`mpv` and `yt-dlp` must be on PATH for manual testing of playback features (`rstube install-deps` installs them). No YouTube API keys — all fetching goes through yt-dlp on public data.
