#!/bin/bash

# Remote installer for catch-pokemon CLI
# Usage: curl -sSL https://raw.githubusercontent.com/matthewmyrick/catch-pokemon/main/install-remote.sh | bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${CYAN}${BOLD}Pokemon Catcher CLI Installer${NC}"
echo ""

# Check for Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Rust/Cargo not found. Installing via rustup...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo -e "${GREEN}Installing catch-pokemon via cargo...${NC}"
cargo install --git https://github.com/matthewmyrick/catch-pokemon

echo ""
echo -e "${GREEN}Setting up shell functions...${NC}"
catch-pokemon setup

echo ""
echo -e "${GREEN}${BOLD}Installation complete!${NC}"
echo -e "${CYAN}Restart your terminal, then try:${NC}"
echo -e "  ${YELLOW}pokemon_encounter${NC}  - Meet a wild Pokemon"
echo -e "  ${YELLOW}catch${NC}              - Throw a Pokeball"
echo -e "  ${YELLOW}pc${NC}                 - View your collection"
