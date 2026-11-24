# TOFIX

Findings from a code scan on 2026-10-04.

## High

- `src/config.rs:58` - `toml::from_str(&bytes).unwrap_or_default()` turns a config file with any parse error (e.g. a hand-edit typo, which `docs/src/playlists.md:82` invites) into an empty `Config`, and the next `playlists add`/`videos add`/`remove` calls `config::save` (`src/config.rs:70`) and overwrites the user's whole config with just the one new entry. Fail loudly on a parse error (return `Result` from `load`) instead of silently defaulting.

## Medium

- `src/tui.rs:319` - the picker's `d` key only calls `state::delete_position` and drops the row in memory; history lines stay, so in `play partial`/`forget partial` the video comes back on the next run (`latest_session_per_video`, `src/tui.rs:152`, classifies from history first). Docs disagree with each other and with the code: `docs/src/commands.md:128` and `docs/src/getting-started.md:93-94` say `d` deletes from history, `docs/src/history.md:41-44` says position only. Decide the intended behavior, implement it, and make the three docs agree.
- `src/main.rs:258` - the "nothing configured" error tells the user to run `rstube videos add <name> <url-or-id>`, but `videos add` takes no name (`src/main.rs:172-181`); following the hint fails with a clap error. Drop `<name>`.
- `CLAUDE.md:11` - says "CI runs no tests or lints (workflows only build releases on tags and deploy docs)", but `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (line 94) and `cargo nextest run` (line 112) on every branch push, and releases are driven by the release commit, not tags. Rewrite the section to match the workflow.
- `docs/src/installation.md:37` - "see `rust-toolchain.toml`", but the repo has no `rust-toolchain.toml`. Remove the reference or state the actual requirement (edition 2024 stable).
- `docs/src/commands.md:134-140` - `history` is documented as "most recent first" with format `[pos/dur (pct%)] Video title`; `show_history` (`src/main.rs:855-893`) prints oldest-to-newest (sessions sorted ascending by `ts_start`, last N shown in order) and includes the video id and an `[unclean exit]` marker. The `-v/--verbose` flag (`src/main.rs:100-102`) is undocumented. Update the doc.
- `src/mpv.rs:20` - `RESUME_TAIL_MARGIN_SECS = 10.0` disagrees with `PARTIAL_TAIL_MARGIN_SECS = 30.0` (`src/tui.rs:45`): a video stopped 10-30s before the end is classified "finished" yet `play any` resumes it a few seconds before the end. `MIN_RESUME_SECS` (`src/mpv.rs:19`) also duplicates `MIN_PARTIAL_SECS`. Use one set of constants for both classification and resume.

## Low

- `docs/src/commands.md:10` - documents `show finished [-d]` / `--details` (lines 70-74 too), which is the hidden deprecated alias (`src/main.rs:113-115`); the real `-v/--verbose` and `--json` flags on `show finished`/`show partial`/`show new` are not mentioned, and the verbose format also prints a date. Document the current flags.
- `docs/src/commands.md:85-89` - `show new` format is given as `[duration] title`; the code prints `[dur] <id> <title>` (`src/main.rs:803`). Fix the doc.
- `docs/src/mpv-config.md:20-21` - refers to `rstube play resume`, a command that does not exist (it is `play partial`). Same in `docs/src/history.md:195-196`.
- `src/main.rs:297-301` - `play any` prints "N total videos across M playlists" with M = `cfg.playlists.len()`, ignoring configured videos that are part of the merged set; it also re-reads the config already loaded in `load_merged_playlists`. Count both sources.
- `src/state.rs:488-491` - `path_exists` is unused and kept alive with `#[allow(dead_code)]`. Delete it.
- `src/tui.rs:440` - `fmt_dur` is a verbatim copy of `src/main.rs:1045`. Keep one shared helper.
