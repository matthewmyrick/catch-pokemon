#!/bin/bash

# Uninstaller for catch-pokemon CLI
# Usage: curl -sSL https://raw.githubusercontent.com/matthewmyrick/catch-pokemon/main/uninstall.sh | bash
#    or: ./uninstall.sh
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${CYAN}${BOLD}Pokemon Catcher CLI Uninstaller${NC}"
echo ""

# Detect shell config
USER_SHELL=$(basename "$SHELL")
if [[ "$USER_SHELL" == "zsh" ]]; then
    SHELL_CONFIG="$HOME/.zshrc"
elif [[ "$USER_SHELL" == "bash" ]]; then
    SHELL_CONFIG="$HOME/.bashrc"
else
    SHELL_CONFIG="$HOME/.profile"
fi

# Remove binary
if [ -f "$HOME/.local/bin/catch-pokemon" ]; then
    rm -f "$HOME/.local/bin/catch-pokemon"
    echo -e "${GREEN}Removed binary from ~/.local/bin/catch-pokemon${NC}"
elif [ -f "$HOME/.cargo/bin/catch-pokemon" ]; then
    rm -f "$HOME/.cargo/bin/catch-pokemon"
    echo -e "${GREEN}Removed binary from ~/.cargo/bin/catch-pokemon${NC}"
else
    echo -e "${YELLOW}Binary not found (already removed?)${NC}"
fi

# Remove shell functions
FUNCTIONS_DIR="$HOME/.local/share/catch-pokemon"
if [ -d "$FUNCTIONS_DIR" ]; then
    # Keep pc_storage.json if it exists (don't delete Pokemon!)
    if [ -f "$FUNCTIONS_DIR/functions.sh" ]; then
        rm -f "$FUNCTIONS_DIR/functions.sh"
        echo -e "${GREEN}Removed shell functions${NC}"
    fi
else
    # Check macOS path
    FUNCTIONS_DIR="$HOME/Library/Application Support/catch-pokemon"
    if [ -f "$FUNCTIONS_DIR/functions.sh" ]; then
        rm -f "$FUNCTIONS_DIR/functions.sh"
        echo -e "${GREEN}Removed shell functions${NC}"
    else
        echo -e "${YELLOW}Shell functions not found (already removed?)${NC}"
    fi
fi

# Remove source line from shell config
if [ -f "$SHELL_CONFIG" ]; then
    if grep -q "catch-pokemon" "$SHELL_CONFIG"; then
        # Create backup
        cp "$SHELL_CONFIG" "$SHELL_CONFIG.backup-uninstall"
        # Remove catch-pokemon lines
        grep -v "catch-pokemon" "$SHELL_CONFIG" > "$SHELL_CONFIG.tmp"
        mv "$SHELL_CONFIG.tmp" "$SHELL_CONFIG"
        echo -e "${GREEN}Removed source line from $SHELL_CONFIG${NC}"
        echo -e "${YELLOW}Backup saved to $SHELL_CONFIG.backup-uninstall${NC}"
    else
        echo -e "${YELLOW}No catch-pokemon lines found in $SHELL_CONFIG${NC}"
    fi
fi

echo ""
echo -e "${GREEN}${BOLD}Uninstall complete!${NC}"
echo ""
echo -e "${CYAN}Note:${NC}"
echo -e "  - Your caught Pokemon data was ${BOLD}NOT${NC} deleted."
echo -e "  - To delete your Pokemon collection too, run:"

# Show correct storage path
if [ -d "$HOME/Library/Application Support/catch-pokemon" ]; then
    echo -e "    ${YELLOW}rm -rf \"$HOME/Library/Application Support/catch-pokemon\"${NC}"
elif [ -d "$HOME/.local/share/catch-pokemon" ]; then
    echo -e "    ${YELLOW}rm -rf $HOME/.local/share/catch-pokemon${NC}"
fi

echo -e "  - Restart your terminal to complete the uninstall."
