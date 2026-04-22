# Installation

## Download a pre-built binary

Pre-built binaries are published for each tagged release on GitHub.

```bash
# x86_64 linux
gh release download latest --repo veltzer/rstube \
  --pattern 'rstube-linux-x86_64' --output rstube --clobber

# aarch64 linux
gh release download latest --repo veltzer/rstube \
  --pattern 'rstube-linux-aarch64' --output rstube --clobber

# macOS x86_64
gh release download latest --repo veltzer/rstube \
  --pattern 'rstube-macos-x86_64' --output rstube --clobber

# macOS arm64
gh release download latest --repo veltzer/rstube \
  --pattern 'rstube-macos-aarch64' --output rstube --clobber

chmod +x rstube
sudo mv rstube /usr/local/bin/
```

## Build from source

```bash
git clone https://github.com/veltzer/rstube.git
cd rstube
cargo build --release
sudo install -m 755 target/release/rstube /usr/local/bin/
```

Requires a reasonably recent stable Rust (see `rust-toolchain.toml`).

## Install runtime dependencies

rstube shells out to two tools:

- `mpv` — video player
- `yt-dlp` — fetches the playlist and resolves video URLs

Install whatever is missing:

```bash
rstube install-deps
```

This auto-detects your package manager (`apt-get`, `dnf`, `pacman`,
`zypper`, or `brew`) for mpv, and prefers `pipx` over `pip --user` for
yt-dlp. Already-installed tools are left alone.

If you'd rather install manually:

```bash
# Debian/Ubuntu
sudo apt-get install -y mpv
pipx install yt-dlp

# Arch
sudo pacman -S --noconfirm mpv
pipx install yt-dlp

# macOS
brew install mpv
pipx install yt-dlp
```

## Install shell completion

Bash:

```bash
rstube complete bash > ~/.local/share/bash-completion/completions/rstube
```

Or source it on the fly:

```bash
source <(rstube complete bash)
```

Other shells — zsh, fish, elvish, powershell — are supported the same way.
See [shell completion](completion.md).
