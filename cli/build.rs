use rand::Rng;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_secret.rs");

    // Priority 1: Use BUILD_SECRET_KEY env var if set (for CI and stable local builds)
    // Accepts any string — it gets hashed into 32 bytes
    let secret: [u8; 32] = if let Ok(key_str) = env::var("BUILD_SECRET_KEY") {
        let trimmed = key_str.trim().to_string();
        assert!(!trimmed.is_empty(), "BUILD_SECRET_KEY must not be empty");
        hash_key(&trimmed)
    } else {
        // Priority 2: Reuse cached key from a previous build if it exists
        let cache_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join(".build_secret");
        if cache_path.exists() {
            if let Ok(cached) = fs::read_to_string(&cache_path) {
                let trimmed = cached.trim().to_string();
                if !trimmed.is_empty() {
                    eprintln!("cargo:warning=Reusing cached build secret from .build_secret");
                    hash_key(&trimmed)
                } else {
                    generate_and_cache(&cache_path)
                }
            } else {
                generate_and_cache(&cache_path)
            }
        } else {
            // Priority 3: Generate a new key and cache it
            generate_and_cache(&cache_path)
        }
    };

    let bytes_str = secret
        .iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ");

    // API URL — hardcoded into binary at build time
    let api_url = env::var("CATCH_POKEMON_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let code = format!(
        "const BUILD_SECRET: [u8; 32] = [{}];\nconst API_URL: &str = \"{}\";\n",
        bytes_str, api_url
    );

    fs::write(&dest_path, code).unwrap();

    // Always rerun to ensure the secret is never stale
    println!("cargo:rerun-if-env-changed=BUILD_SECRET_KEY");
    println!("cargo:rerun-if-env-changed=CATCH_POKEMON_API_URL");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.build_secret");
}

/// Hash any string into 32 bytes using a simple but effective mixing function.
/// Uses multiple rounds of byte mixing to produce a well-distributed key
/// without requiring an external SHA crate in build-dependencies.
fn hash_key(input: &str) -> [u8; 32] {
    let bytes = input.as_bytes();
    let mut state: [u8; 32] = [0u8; 32];

    // Initialize with input bytes spread across state
    for (i, &b) in bytes.iter().enumerate() {
        state[i % 32] ^= b;
        state[(i + 13) % 32] = state[(i + 13) % 32].wrapping_add(b);
        state[(i + 7) % 32] = state[(i + 7) % 32].wrapping_mul(b | 1);
    }

    // Mix thoroughly — 256 rounds
    for round in 0u16..256 {
        for i in 0..32 {
            let prev = state[(i + 31) % 32];
            let next = state[(i + 1) % 32];
            state[i] = state[i]
                .wrapping_add(prev.rotate_left(3))
                .wrapping_add(next.rotate_right(2))
                .wrapping_add(round as u8)
                .wrapping_add(i as u8);
        }
    }

    state
}

fn generate_and_cache(cache_path: &Path) -> [u8; 32] {
    let mut rng = rand::thread_rng();
    // Generate alphanumeric key (a-z, A-Z, 0-9)
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        .chars()
        .collect();
    let key: String = (0..64).map(|_| chars[rng.gen_range(0..chars.len())]).collect();

    eprintln!("cargo:warning=Generated new build secret, cached to .build_secret");
    eprintln!("cargo:warning=Add this to your shell and GitHub secrets as BUILD_SECRET_KEY:");
    eprintln!("cargo:warning={}", key);
    let _ = fs::write(cache_path, &key);
    hash_key(&key)
}
