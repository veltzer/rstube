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

### Playlist-name completion

The bash script injects a small helper that reads your config at tab
time. These tab-complete to the names you've configured in
`playlists add`:

- `rstube playlists show <TAB>`
- `rstube playlists remove <TAB>`
- `rstube playlists fetch <TAB>`

The helper reads `$RSTUBE_CONFIG_DIR/config.toml`, falling back to the
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
