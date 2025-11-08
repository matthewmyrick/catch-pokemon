# Catch Pokemon CLI Game

A terminal-based Pokemon catching game that uses `pokemon-colorscripts` to display Pokemon ASCII art and simulates the classic Pokemon catching mechanics with animated ASCII Pokeball art and a PC storage system.

## Features

- **Pokemon Catching**: Catch Pokemon with different types of Pokeballs
- **Multiple Pokeball Types**: Regular, Great, Ultra, and Master balls with increasing catch rates
- **Pokemon-Specific Catch Rates**: Different Pokemon have different catch difficulties based on their rarity
- **PC Storage System**: All caught Pokemon are stored in your PC with detailed statistics
- **View Collection**: Display all your caught Pokemon with counts and catch history by ball type
- **Release Pokemon**: Release Pokemon back to the wild (single or multiple at once)
- **Check Pokemon Status**: Verify if you've caught a Pokemon before and see catch details
- **Animated ASCII Pokeball**: Watch detailed ASCII art pokeballs shake left and right during catch attempts
- **Dynamic Wiggle Count**: More wiggles for harder-to-catch Pokemon (2-4 based on catch chance)
- **Catch Result Animations**: Unique ASCII art for successful catches (with stars) and escapes (pokeball opens)
- **Hide Pokemon Option**: Choose whether to display the Pokemon when it appears
- **Skip Animation Option**: Fast mode for quick catching without animations
- **Highlighted Catch Rates**: Catch percentages displayed in bright colors for visibility

## Installation

### Prerequisites

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

### Method 1: Cargo Install (Easiest)

Install directly using Cargo:

```bash
# Install from GitHub (available now)
cargo install --git https://github.com/matthewmyrick/catch-pokemon

# Or install from crates.io (once published)
cargo install catch-pokemon
```

This automatically:
- Downloads and compiles the latest version
- Installs to `~/.cargo/bin/catch-pokemon`
- Makes `catch-pokemon` available globally

**After installation, verify it works:**
```bash
catch-pokemon --version
```

**If you get "command not found":** Make sure `~/.cargo/bin` is in your PATH (see Rust installation instructions above)

### Method 2: Build and Install Script

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
- Allow you to use `catch-pokemon` from anywhere in your terminal

**Note:** After running the install script, either restart your terminal or run:
```bash
source ~/.zshrc  # or source ~/.bashrc
```

### Method 3: Manual Build from Source

```bash
git clone https://github.com/matthewmyrick/catch-pokemon.git
cd catch-pokemon
cargo build --release
```

The binary will be available at `target/release/catch-pokemon`. You can then:
- Run it directly: `./target/release/catch-pokemon`
- Copy it to a directory in your PATH: `cp target/release/catch-pokemon ~/.local/bin/`

## Usage

### Catch a Pokemon

```bash
# Catch with a regular Pokeball (default)
catch-pokemon catch pikachu

# Catch with a specific ball type
catch-pokemon catch mewtwo --ball ultra

# Skip animation for faster catching
catch-pokemon catch eevee --ball great --skip-animation

# Hide the Pokemon when it appears (only show the catching animation)
catch-pokemon catch pikachu --hide-pokemon
```

Ball types available:
- `pokeball` or `poke` - Regular Pokéball (1x catch rate) - Red
- `great` or `greatball` - Great Ball (1.5x catch rate) - Blue
- `ultra` or `ultraball` - Ultra Ball (2x catch rate) - Yellow
- `master` or `masterball` - Master Ball (guaranteed catch) - Magenta

### View Your PC

Display all Pokemon you've caught:

```bash
catch-pokemon pc
```

This shows:
- Pokemon grouped by name with total counts
- Breakdown of catches by ball type for each Pokemon
- Summary statistics of total catches by ball type
- Recent catch history with timestamps and ball used
- Highlighted catch rate display

### Release Pokemon

Release Pokemon from your PC back to the wild:

```bash
# Release a single Pokemon
catch-pokemon release pikachu

# Release multiple Pokemon of the same type
catch-pokemon release pikachu --number 3
catch-pokemon release rattata -n 5
```

### Check Pokemon Status

Check if you've caught a specific Pokemon before:

```bash
# Detailed status
catch-pokemon status charizard

# Simple true/false output (useful for scripts)
catch-pokemon status charizard --boolean
```

The detailed mode shows:
- Whether you've caught this Pokemon before
- How many you have in your PC
- Details of your most recent catch

The boolean mode simply returns `true` or `false` for easy scripting.

### Clear PC Storage

Start fresh by clearing all caught Pokemon:

```bash
catch-pokemon clear
```

## Pokemon Catch Rates

The game implements realistic catch rates based on Pokemon rarity:

### Very Hard to Catch (3% base rate)
- **Legendary Pokemon**: Articuno, Zapdos, Moltres, Mewtwo, Lugia, Ho-oh, Rayquaza, Dialga, Palkia, etc.
- **Mythical Pokemon**: Mew, Celebi, Jirachi, Deoxys, Arceus, etc.

### Hard to Catch (45% base rate)
- **Pseudo-Legendary**: Dragonite, Tyranitar, Salamence, Garchomp, etc.
- **Starter Pokemon**: Bulbasaur, Charmander, Squirtle, and all other starters
- **Eevee** and its evolutions

### Moderate Difficulty (120-190% base rate)
- **Pikachu**: 190% base rate (easier than most)
- **Most common Pokemon**: 120% base rate

### Easy to Catch (255% base rate - max)
- **Common Pokemon**: Pidgey, Rattata, Caterpie, Weedle, etc.

## How Catch Rates Work

The final catch chance is calculated as:
```
catch_chance = (pokemon_base_rate × ball_modifier) / 255 × 100%
```

Example: Catching Mewtwo with an Ultra Ball
- Mewtwo base rate: 3
- Ultra Ball modifier: 2.0
- Catch chance: (3 × 2.0) / 255 × 100% = 2.35%

## Storage Location

Caught Pokemon are stored persistently in:
- **macOS/Linux**: `~/.local/share/catch-pokemon/pc_storage.json`
- **Windows**: `%LOCALAPPDATA%\catch-pokemon\pc_storage.json`

## Examples

```bash
# Try to catch a legendary with an Ultra Ball
catch-pokemon catch articuno --ball ultra

# Guaranteed catch with Master Ball
catch-pokemon catch rayquaza --ball master

# Catch a starter Pokemon without showing it
catch-pokemon catch charmander --ball great --hide-pokemon

# Quick catch without animations
catch-pokemon catch rattata --skip-animation

# Check your collection
catch-pokemon pc

# Check if you've caught a Pokemon before
catch-pokemon status mewtwo

# Release a Pokemon back to the wild
catch-pokemon release pidgey

# Release multiple Pokemon at once
catch-pokemon release rattata --number 10

# Clear your PC and start over
catch-pokemon clear
```

## Command Help

```bash
# See all available commands
catch-pokemon --help

# Get help for a specific command
catch-pokemon catch --help
catch-pokemon release --help
catch-pokemon status --help
```

## Animation Details

The catching sequence includes:
1. Pokemon appears (shown via pokemon-colorscripts, optional with `--hide-pokemon`)
2. "You throw a Poké Ball!" message
3. ASCII art pokeball appears and shakes left-right-left-center (2-4 times based on catch difficulty)
4. Final result: Either pokeball with stars (success) or opened pokeball (escape)
5. Success/failure message and PC storage confirmation

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run directly with cargo
cargo run -- catch pikachu
```

## Dependencies

- `clap` - Command line argument parsing
- `colored` - Terminal color output  
- `rand` - Random number generation
- `crossterm` - Terminal manipulation for animations
- `serde` & `serde_json` - PC storage serialization
- `chrono` - Timestamp tracking
- `dirs` - Cross-platform directory paths

## License

MIT

## Contributing

Pull requests are welcome! Ideas for improvements:
- Add more Pokeball types (Quick Ball, Timer Ball, etc.)
- Implement shiny Pokemon with special colors
- Add battle system before catching
- Create trading functionality between users
- Add Pokemon stats and levels

## Acknowledgments

- [pokemon-colorscripts](https://gitlab.com/phoneybadger/pokemon-colorscripts) for the amazing Pokemon ASCII art
- The Pokemon franchise for the inspiration