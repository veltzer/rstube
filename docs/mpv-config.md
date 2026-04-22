# mpv configuration tips

rstube shells out to `mpv` to play videos — it doesn't manage mpv's settings.
Customize mpv by editing its config files directly. On Linux these live under
`~/.config/mpv/`.

## Position save/restore

rstube tracks positions itself (in `$XDG_STATE_HOME/rstube/positions.json`) and
passes `--start=<secs>` to mpv when resuming. You don't need mpv's own
save-position-on-quit for rstube's resume flow.

If you also use mpv outside rstube and want mpv to remember positions on its
own, add this to `~/.config/mpv/mpv.conf`:

```
save-position-on-quit=yes
```

The two mechanisms don't conflict, but only rstube's is visible to `rstube play
resume`.

## Mouse bindings (click to pause, etc.)

mpv has no mouse bindings by default beyond the on-screen controller. To make
left-click pause/play, add these to `~/.config/mpv/input.conf` (create the file
if it doesn't exist):

```
MBTN_LEFT     cycle pause
MBTN_RIGHT    cycle mute
WHEEL_UP      seek 10
WHEEL_DOWN    seek -10
```

- `MBTN_LEFT` → single-click pauses/resumes. Double-click still goes fullscreen
  — mpv disambiguates single vs double on its own.
- `MBTN_RIGHT` → toggles mute.
- `WHEEL_UP` / `WHEEL_DOWN` → seek ±10s.

Restart mpv (close and reopen) for changes to take effect. No rstube restart
needed — rstube just spawns `mpv`, so it picks up whatever config is on disk at
spawn time.

## Full keybind reference

Run `mpv --input-keylist` to see every bindable key name, and `mpv
--input-cmdlist` for every command. The online reference is at
[mpv.io/manual](https://mpv.io/manual/stable/#input-conf).

## Why rstube doesn't ship mpv config

Two reasons:

1. **Your mpv, your rules.** Baking bindings into rstube would override
   whatever you have in `input.conf` when launched via rstube, but not
   otherwise — inconsistent and surprising.
2. **mpv's config is already the right place for mpv settings.** Anyone using
   mpv already has a workflow for managing it; rstube duplicating that as
   command-line flags would be friction for no gain.
