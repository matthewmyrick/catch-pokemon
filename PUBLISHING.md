# Publishing Guide

This document explains how to publish `catch-pokemon` to crates.io.

## Prerequisites

1. Create an account at [crates.io](https://crates.io)
2. Get your API token from [crates.io/me](https://crates.io/me)

## Publishing Steps

You have two options for publishing: automatic (via GitHub Actions) or manual.

### Option 1: Automatic Publishing (Recommended)

The project includes a GitHub Actions workflow that automatically publishes to crates.io when you push a new version.

#### Setup (One-time)

1. Create an account at [crates.io](https://crates.io)
2. Get your API token from [crates.io/me](https://crates.io/me)
3. Add the token to your GitHub repository:
   - Go to your repo Settings → Secrets and variables → Actions
   - Click "New repository secret"
   - Name: `CARGO_REGISTRY_TOKEN`
   - Value: Your crates.io API token
   - Click "Add secret"

#### Releasing a New Version

1. Update the version in `Cargo.toml`:
   ```toml
   version = "1.0.1"
   ```

2. Commit and push:
   ```bash
   git add Cargo.toml
   git commit -m "Bump version to 1.0.1"
   git push
   ```

3. The GitHub Action will automatically:
   - Create a git tag (v1.0.1)
   - Build binaries for Linux, macOS, and Windows
   - Create a GitHub Release with the binaries
   - Publish to crates.io (if CARGO_REGISTRY_TOKEN is set)

### Option 2: Manual Publishing

If you prefer to publish manually:

#### 1. Login to Cargo

```bash
cargo login
# Paste your API token when prompted
```

#### 2. Verify the Package

Check what will be included in the package:

```bash
cargo package --list
```

#### 3. Test the Package Build

Build the package locally to ensure it works:

```bash
cargo package --allow-dirty
```

This creates a `.crate` file in `target/package/`.

#### 4. Commit Your Changes

Make sure all changes are committed to git:

```bash
git add Cargo.toml README.md
git commit -m "Prepare for crates.io publishing"
git push
```

#### 5. Publish to crates.io

```bash
cargo publish
```

#### 6. Create Git Tag (if not using automatic releases)

```bash
VERSION=$(grep "^version =" Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
git tag -a v$VERSION -m "Release v$VERSION"
git push origin v$VERSION
```

## After Publishing

Once published, users can install with:

```bash
cargo install catch-pokemon
```

## Version Updates

When releasing a new version:

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md` (if you have one)
3. Commit the changes
4. Create a git tag:
   ```bash
   git tag -a v1.0.1 -m "Release v1.0.1"
   git push origin v1.0.1
   ```
5. Publish to crates.io:
   ```bash
   cargo publish
   ```

## Installation Methods Summary

After publishing, users will have three installation options:

### 1. From crates.io (Recommended for users)
```bash
cargo install catch-pokemon
```

### 2. From GitHub
```bash
cargo install --git https://github.com/matthewmyrick/catch-pokemon
```

### 3. Using install.sh (For local development)
```bash
git clone https://github.com/matthewmyrick/catch-pokemon.git
cd catch-pokemon
./install.sh
```

## Troubleshooting

### Name Already Taken

If the crate name is already taken on crates.io, you'll need to:
1. Choose a different name (e.g., `pokemon-catcher`, `catch-em-cli`)
2. Update the `name` field in `Cargo.toml`
3. Update all references in README.md
4. Try publishing again

### Failed to Publish

Common issues:
- Missing README.md: Make sure it exists
- Uncommitted changes: Commit all changes first
- Version already published: Bump the version number
- Missing license file: Ensure LICENSE file exists

## Current Status

- ✅ Cargo.toml configured with metadata
- ✅ README.md included
- ✅ All assets embedded in binary
- ✅ Build verified
- ✅ Git install works
- ⏳ Ready to publish to crates.io (when you're ready)
