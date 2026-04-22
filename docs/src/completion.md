# Shell completion

`rstube complete <shell>` prints a completion script to stdout. Supported
shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

## Bash

Install once:

```bash
rstube complete bash > ~/.local/share/bash-completion/completions/rstube
```

Or source on the fly in your `~/.bashrc`:

```bash
source <(rstube complete bash)
```

### Name completion for playlists and videos

The bash script injects small helpers that read your config at tab
time. Completion is scoped by section — playlist names never bleed
into video slots and vice versa.

Playlist names (configured via `playlists add`) complete in:

- `rstube playlists show <TAB>`
- `rstube playlists remove <TAB>`
- `rstube playlists fetch <TAB>`

Video names (configured via `videos add`) complete in:

- `rstube videos show <TAB>`
- `rstube videos remove <TAB>`
- `rstube videos fetch <TAB>`

The helpers read `$RSTUBE_CONFIG_DIR/config.toml`, falling back to the
XDG default.

## Zsh

```bash
rstube complete zsh > ~/.zfunc/_rstube
# ensure ~/.zfunc is in fpath; e.g. in .zshrc:
#   fpath=(~/.zfunc $fpath)
#   autoload -U compinit && compinit
```

## Fish

```bash
rstube complete fish > ~/.config/fish/completions/rstube.fish
```

## Elvish / PowerShell

See the respective shells' documentation for where to install generated
completion scripts. rstube just prints to stdout — installation location
is up to you.

## Notes

- The bash playlist-name injection is bash-specific. Other shells get
  standard clap-generated completion (subcommands, flags, enums) but not
  playlist-name completion.
- Re-run `rstube complete bash > ...` whenever you upgrade rstube — the
  flag set may have changed.
