#!/bin/bash

# Install script for catch-pokemon CLI tool
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Header
echo -e "${CYAN}${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}${BOLD}║         Pokemon Catcher CLI Installer       ║${NC}"
echo -e "${CYAN}${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}Installing catch-pokemon CLI tool...${NC}"
echo ""

# Check if Rust/Cargo is installed
echo -e "${YELLOW}Checking prerequisites...${NC}"
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Error: Rust/Cargo is not installed.${NC}"
    echo -e "${YELLOW}Please install Rust from https://rustup.rs/ and try again.${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Rust/Cargo found${NC}"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}❌ Error: Cargo.toml not found.${NC}"
    echo -e "${YELLOW}Please run this script from the catch-pokemon directory.${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Project directory verified${NC}"

# Show current version
if [ -f "Cargo.toml" ]; then
    VERSION=$(grep "^version =" Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
    echo -e "${BLUE}📦 Version: ${VERSION}${NC}"
fi

echo ""
echo -e "${YELLOW}🔨 Building optimized release binary...${NC}"
cargo build --release

# Get the user's local bin directory
BIN_DIR="$HOME/.local/bin"

# Create the bin directory if it doesn't exist
echo -e "${YELLOW}📁 Creating installation directory...${NC}"
mkdir -p "$BIN_DIR"

# Copy the binary
echo -e "${YELLOW}📋 Installing binary to $BIN_DIR...${NC}"
cp target/release/catch-pokemon "$BIN_DIR/"

# Make sure it's executable
chmod +x "$BIN_DIR/catch-pokemon"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo -e "${YELLOW}🔧 Configuring PATH...${NC}"
    
    # Detect shell and add to appropriate config file
    # Check user's login shell, not the script's execution shell
    USER_SHELL=$(basename "$SHELL")

    if [[ "$USER_SHELL" == "zsh" ]]; then
        SHELL_CONFIG="$HOME/.zshrc"
        SHELL_NAME="Zsh"
    elif [[ "$USER_SHELL" == "bash" ]]; then
        SHELL_CONFIG="$HOME/.bashrc"
        SHELL_NAME="Bash"
    else
        SHELL_CONFIG="$HOME/.profile"
        SHELL_NAME="Shell"
    fi
    
    echo "" >> "$SHELL_CONFIG"
    echo "# Added by catch-pokemon installer" >> "$SHELL_CONFIG"
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_CONFIG"
    
    echo -e "${GREEN}✅ Added to PATH in $SHELL_CONFIG ($SHELL_NAME)${NC}"
    echo -e "${YELLOW}⚠️  Please restart your terminal or run: ${CYAN}source $SHELL_CONFIG${NC}"
else
    echo -e "${GREEN}✅ PATH already configured${NC}"
fi

# Install shell functions
SHELL_FUNCTIONS_DIR="$HOME/.local/share/catch-pokemon"
SHELL_FUNCTIONS_SRC="$(cd "$(dirname "$0")" && pwd)/shell/functions.sh"

echo -e "${YELLOW}🐚 Installing shell functions...${NC}"
mkdir -p "$SHELL_FUNCTIONS_DIR"
cp "$SHELL_FUNCTIONS_SRC" "$SHELL_FUNCTIONS_DIR/functions.sh"
chmod +x "$SHELL_FUNCTIONS_DIR/functions.sh"
echo -e "${GREEN}✅ Shell functions installed to $SHELL_FUNCTIONS_DIR${NC}"

# Add source line to shell config if not already present
SOURCE_LINE="source \"$SHELL_FUNCTIONS_DIR/functions.sh\""
USER_SHELL=$(basename "$SHELL")

if [[ "$USER_SHELL" == "zsh" ]]; then
    SHELL_CONFIG="$HOME/.zshrc"
elif [[ "$USER_SHELL" == "bash" ]]; then
    SHELL_CONFIG="$HOME/.bashrc"
else
    SHELL_CONFIG="$HOME/.profile"
fi

if ! grep -qF "catch-pokemon/functions.sh" "$SHELL_CONFIG" 2>/dev/null; then
    echo "" >> "$SHELL_CONFIG"
    echo "# catch-pokemon shell functions (catch, pokemon_encounter, etc.)" >> "$SHELL_CONFIG"
    echo "$SOURCE_LINE" >> "$SHELL_CONFIG"
    echo -e "${GREEN}✅ Shell functions added to $SHELL_CONFIG${NC}"
    echo -e "${YELLOW}⚠️  Restart your terminal or run: ${CYAN}source $SHELL_CONFIG${NC}"
else
    echo -e "${GREEN}✅ Shell functions already configured in $SHELL_CONFIG${NC}"
fi

echo ""
echo -e "${GREEN}${BOLD}🎉 Installation complete!${NC}"
echo -e "${GREEN}You can now use '${CYAN}catch-pokemon${GREEN}' from anywhere in your terminal.${NC}"
echo ""

echo -e "${BLUE}${BOLD}📚 Quick Start Guide:${NC}"
echo ""
echo -e "${CYAN}Basic Commands:${NC}"
echo -e "  ${YELLOW}catch-pokemon catch pikachu${NC}           # Catch a Pokemon with a regular Pokeball"
echo -e "  ${YELLOW}catch-pokemon catch mewtwo --ball master${NC} # Use a Master Ball for guaranteed catch"
echo -e "  ${YELLOW}catch-pokemon pc${NC}                      # View your Pokemon collection"
echo -e "  ${YELLOW}catch-pokemon status charizard${NC}        # Check if you've caught a Pokemon"
echo -e "  ${YELLOW}catch-pokemon release pidgey -n 5${NC}     # Release 5 Pidgey back to the wild"
echo ""

echo -e "${CYAN}Advanced Options:${NC}"
echo -e "  ${YELLOW}catch-pokemon catch eevee --skip-animation${NC}  # Skip animations for faster catching"
echo -e "  ${YELLOW}catch-pokemon catch bulbasaur --hide-pokemon${NC} # Hide Pokemon sprite, show only catching"
echo -e "  ${YELLOW}catch-pokemon status mewtwo --boolean${NC}        # Get true/false output for scripting"
echo ""

echo -e "${CYAN}Pokeball Types:${NC}"
echo -e "  🔴 ${YELLOW}pokeball${NC} - 1x catch rate (default)"
echo -e "  🔵 ${YELLOW}great${NC}    - 1.5x catch rate"
echo -e "  🟡 ${YELLOW}ultra${NC}    - 2x catch rate"
echo -e "  🟣 ${YELLOW}master${NC}   - Guaranteed catch"
echo ""

echo -e "${BLUE}For detailed help on any command:${NC}"
echo -e "  ${YELLOW}catch-pokemon --help${NC}              # Show all commands"
echo -e "  ${YELLOW}catch-pokemon catch --help${NC}        # Help for catch command"
echo -e "  ${YELLOW}catch-pokemon release --help${NC}      # Help for release command"
echo -e "  ${YELLOW}catch-pokemon status --help${NC}       # Help for status command"
echo ""

echo -e "${GREEN}Happy Pokemon catching! 🎮✨${NC}"