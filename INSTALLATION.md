# Installation

## Quick Install (Recommended)

One command installs everything — the binary, pokemon-colorscripts, and shell functions:

```bash
curl -sSL https://raw.githubusercontent.com/matthewmyrick/catch-pokemon/main/install-remote.sh | bash
```

This automatically:
- Downloads the pre-built binary for your platform (macOS/Linux, x86_64/ARM64)
- Installs [pokemon-colorscripts](https://gitlab.com/phoneybadger/pokemon-colorscripts) for Pokemon ASCII sprites
- Sets up shell functions (`catch`, `pc`, `pokemon_encounter`, etc.)
- Configures your `.zshrc` or `.bashrc`

After installation, restart your terminal and you're ready to play:

```bash
pokemon_encounter    # A wild Pokemon appears!
catch                # Throw a Poke Ball
pc                   # View your collection
```

## Supported Platforms

| Platform | Architecture | Binary |
|----------|-------------|--------|
| macOS | Apple Silicon (M1/M2/M3/M4) | macos-arm64 |
| macOS | Intel | macos-x86_64 |
| Linux | x86_64 | linux-x86_64 |
| Windows | x86_64 | windows-x86_64 |

The installer auto-detects your platform. If a pre-built binary isn't available, it falls back to building from source via Cargo.

## Alternative: Cargo Install

If you already have [Rust](https://rustup.rs/) installed:

```bash
cargo install --git https://github.com/matthewmyrick/catch-pokemon
catch-pokemon setup
```

Note: Building from source generates a unique integrity key for your install. Pre-built binaries from GitHub Releases share a common key.

## Alternative: Build from Source

```bash
git clone https://github.com/matthewmyrick/catch-pokemon.git
cd catch-pokemon
chmod +x install.sh
./install.sh
```

## Prerequisites

The installer handles these automatically, but if you need to install manually:

- **Python 3** - Required by pokemon-colorscripts
- **git** - Required to clone pokemon-colorscripts
- **Terminal with true color support** - iTerm2, Terminal.app, GNOME Terminal, etc.

### Install pokemon-colorscripts manually

*Arch/Arch-based:*
```bash
yay -S pokemon-colorscripts-git
```

*Other Linux/macOS:*
```bash
git clone https://gitlab.com/phoneybadger/pokemon-colorscripts.git
cd pokemon-colorscripts
sudo ./install.sh
```

## Shell Commands

After installation, these commands are available:

| Command | Description |
|---------|-------------|
| `pokemon_encounter` | Generate a new wild Pokemon encounter |
| `catch` | Attempt to catch the current wild Pokemon |
| `pc` | View your Pokemon collection |
| `pokemon_status` | Show current encounter status |
| `pokemon_check <name>` | Check if you own a specific Pokemon |
| `pokemon_new` | Force a new encounter |
| `pokemon_help` | Show all available commands |

## Storage Location

Caught Pokemon are stored persistently in:
- **macOS**: `~/Library/Application Support/catch-pokemon/pc_storage.json`
- **Linux**: `~/.local/share/catch-pokemon/pc_storage.json`
- **Windows**: `%LOCALAPPDATA%\catch-pokemon\pc_storage.json`

## Verify Installation

```bash
catch-pokemon --version       # Check binary is installed
pokemon-colorscripts --help   # Check sprites are available
catch-pokemon verify          # Check PC integrity
```
