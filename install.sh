#!/bin/bash

# Install script for catch-pokemon CLI tool
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Installing catch-pokemon CLI tool...${NC}"

# Check if Rust/Cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Rust/Cargo is not installed.${NC}"
    echo "Please install Rust from https://rustup.rs/ and try again."
    exit 1
fi

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Cargo.toml not found. Please run this script from the catch-pokemon directory.${NC}"
    exit 1
fi

echo -e "${YELLOW}Building optimized release...${NC}"
cargo build --release

# Get the user's local bin directory
BIN_DIR="$HOME/.local/bin"

# Create the bin directory if it doesn't exist
mkdir -p "$BIN_DIR"

# Copy the binary
echo -e "${YELLOW}Installing to $BIN_DIR...${NC}"
cp target/release/catch-pokemon "$BIN_DIR/"

# Make sure it's executable
chmod +x "$BIN_DIR/catch-pokemon"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo -e "${YELLOW}Adding $BIN_DIR to PATH...${NC}"
    
    # Detect shell and add to appropriate config file
    if [ -n "$ZSH_VERSION" ]; then
        SHELL_CONFIG="$HOME/.zshrc"
    elif [ -n "$BASH_VERSION" ]; then
        SHELL_CONFIG="$HOME/.bashrc"
    else
        SHELL_CONFIG="$HOME/.profile"
    fi
    
    echo "" >> "$SHELL_CONFIG"
    echo "# Added by catch-pokemon installer" >> "$SHELL_CONFIG"
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_CONFIG"
    
    echo -e "${GREEN}Added to PATH in $SHELL_CONFIG${NC}"
    echo -e "${YELLOW}Please restart your terminal or run: source $SHELL_CONFIG${NC}"
fi

echo -e "${GREEN}✅ Installation complete!${NC}"
echo -e "${GREEN}You can now use 'catch-pokemon' from anywhere in your terminal.${NC}"
echo ""
echo "Examples:"
echo "  catch-pokemon catch pikachu"
echo "  catch-pokemon catch charizard --ball ultra"
echo "  catch-pokemon pc"
echo ""
echo "Run 'catch-pokemon --help' for more options."