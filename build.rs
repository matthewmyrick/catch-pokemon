use rand::Rng;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_secret.rs");

    // Generate a unique 32-byte secret at build time
    // This key is never stored in source code — only in the compiled binary
    let mut rng = rand::thread_rng();
    let secret: [u8; 32] = rng.gen();

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

    // Only regenerate when build.rs changes, not on every source edit
    println!("cargo:rerun-if-changed=build.rs");
}
