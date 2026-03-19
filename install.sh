#!/bin/bash

# Installer for catch-pokemon CLI
# Usage: curl -sSL https://raw.githubusercontent.com/matthewmyrick/catch-pokemon/main/install.sh | bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

REPO="matthewmyrick/catch-pokemon"
BIN_DIR="$HOME/.local/bin"

echo -e "${CYAN}${BOLD}Pokemon Catcher CLI Installer${NC}"
echo ""

# --- Detect OS and architecture ---
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux)  PLATFORM="linux" ;;
    darwin) PLATFORM="macos" ;;
    *)
        echo -e "${RED}Unsupported OS: $OS${NC}"
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64)   ARCH_SUFFIX="x86_64" ;;
    arm64|aarch64)   ARCH_SUFFIX="arm64" ;;
    *)
        echo -e "${RED}Unsupported architecture: $ARCH${NC}"
        exit 1
        ;;
esac

SUFFIX="${PLATFORM}-${ARCH_SUFFIX}"
echo -e "${GREEN}Detected: ${SUFFIX}${NC}"

# --- Get latest release version ---
echo -e "${YELLOW}Fetching latest release...${NC}"
LATEST_TAG=$(curl -sSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
    echo -e "${RED}No release found. Please check https://github.com/$REPO/releases${NC}"
    exit 1
fi

# --- Download pre-built binary ---
ARCHIVE="catch-pokemon-${LATEST_TAG}-${SUFFIX}.tar.gz"
URL="https://github.com/$REPO/releases/download/${LATEST_TAG}/${ARCHIVE}"

echo -e "${GREEN}Downloading ${LATEST_TAG} for ${SUFFIX}...${NC}"

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

if ! curl -sSL --fail -o "$TMPDIR/$ARCHIVE" "$URL"; then
    echo -e "${RED}Download failed. Binary may not be available for ${SUFFIX}.${NC}"
    echo -e "${RED}Check https://github.com/$REPO/releases${NC}"
    exit 1
fi

mkdir -p "$BIN_DIR"
tar -xzf "$TMPDIR/$ARCHIVE" -C "$TMPDIR"
cp "$TMPDIR/catch-pokemon" "$BIN_DIR/"
chmod +x "$BIN_DIR/catch-pokemon"
echo -e "${GREEN}Binary installed to $BIN_DIR/catch-pokemon${NC}"

# --- Ensure ~/.local/bin is in PATH ---
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    USER_SHELL=$(basename "$SHELL")
    if [[ "$USER_SHELL" == "zsh" ]]; then
        SHELL_CONFIG="$HOME/.zshrc"
    elif [[ "$USER_SHELL" == "bash" ]]; then
        SHELL_CONFIG="$HOME/.bashrc"
    else
        SHELL_CONFIG="$HOME/.profile"
    fi

    if ! grep -qF '/.local/bin' "$SHELL_CONFIG" 2>/dev/null; then
        echo "" >> "$SHELL_CONFIG"
        echo '# Added by catch-pokemon installer' >> "$SHELL_CONFIG"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_CONFIG"
        echo -e "${GREEN}Added ~/.local/bin to PATH in $SHELL_CONFIG${NC}"
    fi

    export PATH="$HOME/.local/bin:$PATH"
fi

# --- Install pokemon-colorscripts if not present ---
if ! command -v pokemon-colorscripts &> /dev/null; then
    echo ""
    echo -e "${YELLOW}Installing pokemon-colorscripts (required for Pokemon sprites)...${NC}"

    if ! command -v python3 &> /dev/null; then
        echo -e "${RED}Python 3 is required for pokemon-colorscripts but not found.${NC}"
        echo -e "${YELLOW}Please install Python 3 and re-run this installer.${NC}"
    else
        TMPDIR_PCS=$(mktemp -d)
        trap "rm -rf $TMPDIR_PCS" EXIT

        git clone https://gitlab.com/phoneybadger/pokemon-colorscripts.git "$TMPDIR_PCS/pokemon-colorscripts" 2>/dev/null

        if [ -f "$TMPDIR_PCS/pokemon-colorscripts/install.sh" ]; then
            echo -e "${YELLOW}Installing pokemon-colorscripts (may require sudo)...${NC}"
            cd "$TMPDIR_PCS/pokemon-colorscripts"
            sudo ./install.sh
            cd - > /dev/null
            echo -e "${GREEN}pokemon-colorscripts installed${NC}"
        else
            echo -e "${RED}Failed to clone pokemon-colorscripts.${NC}"
            echo -e "${YELLOW}Install manually: https://gitlab.com/phoneybadger/pokemon-colorscripts${NC}"
        fi
    fi
else
    echo -e "${GREEN}pokemon-colorscripts already installed${NC}"
fi

# --- Set up shell functions ---
echo ""
echo -e "${YELLOW}Setting up shell functions...${NC}"
catch-pokemon setup

echo ""
echo -e "${GREEN}${BOLD}Installation complete!${NC}"
echo ""
echo -e "${CYAN}Restart your terminal, then play:${NC}"
echo -e "  ${YELLOW}pokemon_encounter${NC}  - A wild Pokemon appears!"
echo -e "  ${YELLOW}catch${NC}              - Throw a Poke Ball"
echo -e "  ${YELLOW}pc${NC}                 - View your collection"
echo -e "  ${YELLOW}pokemon_help${NC}       - See all commands"
