#!/usr/bin/env python3
"""
Admin tool: Encrypt a plaintext PC JSON file using the BUILD_SECRET_KEY.
Produces an encrypted pc_storage.json that any official binary can read.

Usage:
    python3 admin-encrypt.py <input.json> <output_encrypted> [BUILD_SECRET_KEY]

    If BUILD_SECRET_KEY is not passed as argument, reads from BUILD_SECRET_KEY env var.

Example:
    python3 admin-encrypt.py user_backup.json pc_storage.json
    python3 admin-encrypt.py user_backup.json pc_storage.json YOUR_BUILD_SECRET_KEY
"""

import hashlib
import hmac
import json
import os
import struct
import sys

# AES-GCM via cryptography library
try:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
except ImportError:
    print("ERROR: Install cryptography package: pip3 install cryptography")
    sys.exit(1)


def hash_key(input_str: str) -> bytes:
    """Reproduce the Rust build.rs hash_key() function exactly."""
    data = input_str.encode()
    state = [0] * 32

    for i, b in enumerate(data):
        state[i % 32] ^= b
        state[(i + 13) % 32] = (state[(i + 13) % 32] + b) & 0xFF
        state[(i + 7) % 32] = (state[(i + 7) % 32] * (b | 1)) & 0xFF

    for round_num in range(256):
        for i in range(32):
            prev = state[(i + 31) % 32]
            nxt = state[(i + 1) % 32]
            state[i] = (
                state[i]
                + rotate_left(prev, 3)
                + rotate_right(nxt, 2)
                + (round_num & 0xFF)
                + i
            ) & 0xFF

    return bytes(state)


def rotate_left(b: int, n: int) -> int:
    return ((b << n) | (b >> (8 - n))) & 0xFF


def rotate_right(b: int, n: int) -> int:
    return ((b >> n) | (b << (8 - n))) & 0xFF


def derive_signing_key(build_secret: bytes) -> bytes:
    """Reproduce the Rust derive_signing_key() — no salt, just BUILD_SECRET."""
    kdf_domain = b"catch-pokemon:kdf:v1"
    sign_domain = b"catch-pokemon:sign:v1"

    # Step 1: Domain-separated intermediate key
    key = hmac.new(build_secret, kdf_domain, hashlib.sha256).digest()

    # Step 2: 10,000 rounds of HMAC stretching
    for round_num in range(10_000):
        round_bytes = struct.pack("<I", round_num)
        mac = hmac.new(key, round_bytes + sign_domain, hashlib.sha256)
        key = mac.digest()

    return key


def derive_encryption_key(signing_key: bytes) -> bytes:
    """Reproduce the Rust derive_encryption_key()."""
    encryption_domain = b"catch-pokemon:encryption:v1"
    return hmac.new(signing_key, encryption_domain, hashlib.sha256).digest()


def sign_entry(key: bytes, entry: dict, prev_hash: str) -> str:
    """Sign a single PC entry."""
    shiny = str(entry.get("shiny", False)).lower()
    canonical = f"{entry['name']}|{entry['caught_at']}|{entry['ball_used']}|{shiny}"
    mac = hmac.new(key, (canonical + prev_hash).encode(), hashlib.sha256)
    return mac.hexdigest()


def compute_entry_hash(entry: dict, prev_hash: str) -> str:
    """Compute chain hash for an entry."""
    chain_domain = b"catch-pokemon:chain:v1"
    shiny = str(entry.get("shiny", False)).lower()
    canonical = f"{entry['name']}|{entry['caught_at']}|{entry['ball_used']}|{shiny}"
    h = hashlib.sha256()
    h.update(chain_domain)
    h.update(canonical.encode())
    h.update(prev_hash.encode())
    return h.hexdigest()


def resign_chain(pc_data: dict, signing_key: bytes) -> dict:
    """Re-sign the entire chain with the given key."""
    prev_hash = "genesis"
    for entry in pc_data.get("pokemon", []):
        entry["prev_hash"] = prev_hash
        entry["signature"] = sign_entry(signing_key, entry, prev_hash)
        prev_hash = compute_entry_hash(entry, prev_hash)

    if pc_data.get("pokemon"):
        pc_data["chain_hash"] = prev_hash
    else:
        pc_data["chain_hash"] = None

    return pc_data


def encrypt(data: bytes, encryption_key: bytes) -> bytes:
    """AES-256-GCM encrypt: output = [12-byte nonce][ciphertext]."""
    nonce = os.urandom(12)
    aesgcm = AESGCM(encryption_key)
    ciphertext = aesgcm.encrypt(nonce, data, None)
    return nonce + ciphertext


def main():
    if len(sys.argv) < 3:
        print("Usage: python3 admin-encrypt.py <input.json> <output_encrypted> [BUILD_SECRET_KEY]")
        sys.exit(1)

    input_path = sys.argv[1]
    output_path = sys.argv[2]

    if len(sys.argv) >= 4:
        secret_str = sys.argv[3]
    else:
        secret_str = os.environ.get("BUILD_SECRET_KEY", "")

    if not secret_str:
        print("ERROR: No BUILD_SECRET_KEY provided (pass as 3rd arg or set env var)")
        sys.exit(1)

    # Read input
    with open(input_path) as f:
        raw = json.load(f)

    # Strip signature wrapper if present
    if "data" in raw and "signature" in raw:
        pc_data = raw["data"]
        print("Stripped signature wrapper from backup")
    else:
        pc_data = raw

    pokemon_count = len(pc_data.get("pokemon", []))
    print(f"Found {pokemon_count} Pokemon")

    # Derive keys
    build_secret = hash_key(secret_str)
    signing_key = derive_signing_key(build_secret)
    encryption_key = derive_encryption_key(signing_key)

    # Re-sign the chain
    pc_data = resign_chain(pc_data, signing_key)
    print("Chain re-signed")

    # Encrypt
    json_bytes = json.dumps(pc_data).encode()
    encrypted = encrypt(json_bytes, encryption_key)

    # Write output
    with open(output_path, "wb") as f:
        f.write(encrypted)

    print(f"Encrypted {pokemon_count} Pokemon -> {output_path}")
    print(f"File size: {len(encrypted)} bytes")
    print("Done. Send this file to the user as pc_storage.json")


if __name__ == "__main__":
    main()
