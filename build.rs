use rand::Rng;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_secret.rs");

    // Priority 1: Use BUILD_SECRET_KEY env var if set (for CI and stable local builds)
    // This should be a 64-char hex string (32 bytes)
    let secret: [u8; 32] = if let Ok(hex_key) = env::var("BUILD_SECRET_KEY") {
        parse_hex_key(&hex_key)
    } else {
        // Priority 2: Reuse cached key from a previous build if it exists
        let cache_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join(".build_secret");
        if cache_path.exists() {
            if let Ok(cached_hex) = fs::read_to_string(&cache_path) {
                let trimmed = cached_hex.trim();
                if trimmed.len() == 64 {
                    eprintln!("cargo:warning=Reusing cached build secret from .build_secret");
                    parse_hex_key(trimmed)
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

    let code = format!(
        "const BUILD_SECRET: [u8; 32] = [{}];\n",
        bytes_str
    );

    fs::write(&dest_path, code).unwrap();

    // Rerun if the env var or build.rs changes
    println!("cargo:rerun-if-env-changed=BUILD_SECRET_KEY");
    println!("cargo:rerun-if-changed=build.rs");
}

fn parse_hex_key(hex: &str) -> [u8; 32] {
    let hex = hex.trim();
    assert!(hex.len() == 64, "BUILD_SECRET_KEY must be exactly 64 hex characters (32 bytes)");
    let mut key = [0u8; 32];
    for i in 0..32 {
        key[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .expect("BUILD_SECRET_KEY must be valid hex");
    }
    key
}

fn generate_and_cache(cache_path: &Path) -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();
    let hex: String = secret.iter().map(|b| format!("{:02x}", b)).collect();
    eprintln!("cargo:warning=Generated new build secret, cached to .build_secret");
    eprintln!("cargo:warning=Add this to your shell and GitHub secrets as BUILD_SECRET_KEY:");
    eprintln!("cargo:warning={}", hex);
    let _ = fs::write(cache_path, &hex);
    secret
}
