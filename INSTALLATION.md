# Installation

## Prerequisites

**Required:**
- **Python 3** - Required by pokemon-colorscripts to display Pokemon ASCII art
- **Terminal with true color support** - Most modern terminals (iTerm2, Terminal.app, GNOME Terminal, etc.) support this
- **[pokemon-colorscripts](https://gitlab.com/phoneybadger/pokemon-colorscripts)** - For displaying Pokemon ASCII art

  **Install pokemon-colorscripts:**

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

**For installation:**
- **Rust 1.70+** - Required for cargo install or building from source

  **Install Rust from [rustup.rs](https://rustup.rs/):**
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

  **IMPORTANT:** After installing Rust, you need to add Cargo's bin directory to your PATH:

  *For zsh (macOS default):*
  ```bash
  echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
  source ~/.zshrc
  ```

  *For bash:*
  ```bash
  echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
  source ~/.bashrc
  ```

  Verify Rust is installed:
  ```bash
  cargo --version
  ```

## Method 1: One-Line Install (Easiest)

Install everything with a single command:

```bash
curl -sSL https://raw.githubusercontent.com/matthewmyrick/catch-pokemon/main/install-remote.sh | bash
```

This automatically:
- Installs Rust if not already present
- Builds and installs `catch-pokemon` via cargo
- Sets up shell functions (`catch`, `pc`, `pokemon_encounter`, etc.)
- Configures your `.zshrc` or `.bashrc`

After installation, restart your terminal and you're ready to play!

## Method 2: Cargo Install

If you already have Rust installed:

```bash
# Install the binary
cargo install --git https://github.com/matthewmyrick/catch-pokemon

# Set up shell functions (catch, pc, pokemon_encounter, etc.)
catch-pokemon setup
```

**If you get "command not found":** Make sure `~/.cargo/bin` is in your PATH (see Rust installation instructions above)

## Method 3: Build and Install Script

Clone the repository and use the installation script:

```bash
git clone https://github.com/matthewmyrick/catch-pokemon.git
cd catch-pokemon
chmod +x install.sh
./install.sh
```

This will:
- Build the optimized release binary
- Install it to `~/.local/bin/catch-pokemon`
- Automatically add `~/.local/bin` to your PATH (in `.zshrc` or `.bashrc`)
- Install shell functions with convenient shortcuts
- Allow you to use `catch-pokemon` from anywhere in your terminal

**Note:** After running the install script, either restart your terminal or run:
```bash
source ~/.zshrc  # or source ~/.bashrc
```

## Method 4: Manual Build from Source

```bash
git clone https://github.com/matthewmyrick/catch-pokemon.git
cd catch-pokemon
cargo build --release
```

The binary will be available at `target/release/catch-pokemon`. You can then:
- Run it directly: `./target/release/catch-pokemon`
- Copy it to a directory in your PATH: `cp target/release/catch-pokemon ~/.local/bin/`
- Set up shell functions: `catch-pokemon setup`

## Shell Commands

After installation, these shell commands are available:

| Command | Description |
|---------|-------------|
| `pokemon_encounter` | Generate a new wild Pokemon encounter |
| `catch` | Attempt to catch the current wild Pokemon |
| `pc` | View your Pokemon collection |
| `pokemon_status` | Show current encounter status |
| `pokemon_check <name>` | Check if you own a specific Pokemon |
| `pokemon_new` | Force a new encounter |
| `pokemon_clear` | Clear current encounter (testing) |
| `pokemon_help` | Show all available commands |

## Storage Location

Caught Pokemon are stored persistently in:
- **macOS**: `~/Library/Application Support/catch-pokemon/pc_storage.json`
- **Linux**: `~/.local/share/catch-pokemon/pc_storage.json`
- **Windows**: `%LOCALAPPDATA%\catch-pokemon\pc_storage.json`

## Dependencies

- `clap` - Command line argument parsing
- `colored` - Terminal color output
- `rand` - Random number generation
- `crossterm` - Terminal manipulation for animations
- `serde` & `serde_json` - PC storage serialization
- `chrono` - Timestamp tracking
- `dirs` - Cross-platform directory paths
- `hmac` & `sha2` - Cryptographic integrity signing
- `hex` - Hex encoding for signatures
- `hostname` - Machine identity for key derivation
