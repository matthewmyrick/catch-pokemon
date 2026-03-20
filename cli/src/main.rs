use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use clap::{Parser, Subcommand};
use colored::*;
use hmac::{Hmac, Mac as HmacMac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;
use chrono::{DateTime, Local};
use crossterm::{
    cursor, terminal, ExecutableCommand,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
};

// Build-time generated secret — never exists in source code
include!(concat!(env!("OUT_DIR"), "/build_secret.rs"));

type HmacSha256 = Hmac<Sha256>;

#[derive(Parser, Debug)]
#[command(
    author, 
    version, 
    about = "Catch Pokemon in your terminal!", 
    long_about = "A fun terminal-based Pokemon catching game with animated ASCII art!\n\n\
Available commands:\n  \
catch     Try to catch a Pokemon with different Pokeball types\n  \
pc        View your Pokemon collection with detailed statistics\n  \
release   Release Pokemon back to the wild\n  \
status    Check if you've caught a Pokemon before\n  \
clear     Clear your entire Pokemon collection\n\n\
Examples:\n  \
catch-pokemon catch pikachu --ball ultra\n  \
catch-pokemon pc\n  \
catch-pokemon status charizard --boolean\n  \
catch-pokemon release rattata --number 5"
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Try to catch a Pokemon with animated Pokeball throwing
    #[command(long_about = "Attempt to catch a Pokemon using a Poke Ball.\n\n\
Each Pokemon has different catch rates based on rarity:\n\
- Legendary/Mythical: 3% base rate (very hard)\n\
- Pseudo-legendary/Starters: 45% base rate (hard)\n\
- Common Pokemon: 120-255% base rate (easy)\n\n\
Examples:\n\
  catch-pokemon catch pikachu\n\
  catch-pokemon catch bulbasaur --hide-pokemon\n\
  catch-pokemon catch charizard --skip-animation")]
    Catch {
        /// Name of the Pokemon to catch (case insensitive)
        pokemon: String,

        /// Skip the animated Pokeball throwing sequence
        #[arg(short = 's', long, help = "Skip animations for faster catching")]
        skip_animation: bool,

        /// Hide the Pokemon ASCII art when it appears
        #[arg(long, default_value = "false", help = "Don't show Pokemon sprite, only catching animation")]
        hide_pokemon: bool,

        /// Mark this Pokemon as shiny (set by encounter system)
        #[arg(long, default_value = "false", hide = true)]
        shiny: bool,

        /// Attempt number for rolling flee rate (set by shell function)
        #[arg(long, default_value = "1", hide = true)]
        attempt: u32,
    },
    
    /// Display your Pokemon collection with detailed statistics
    #[command(long_about = "View all Pokemon you've caught in your PC storage.\n\n\
Shows detailed information including:\n\
- Pokemon grouped by name with total counts\n\
- Breakdown of catches by ball type for each Pokemon\n\
- Summary statistics of total catches by ball type\n\
- Recent catch history with timestamps\n\n\
Examples:\n\
  catch-pokemon pc\n\
  catch-pokemon pc --search\n\
  catch-pokemon pc -s")]
    Pc {
        /// Launch interactive fuzzy search interface
        #[arg(short = 's', long, help = "Launch interactive fuzzy search interface")]
        search: bool,
    },
    
    /// Release Pokemon from your PC back to the wild
    #[command(long_about = "Release Pokemon from your PC storage back to the wild.\n\n\
You can release single Pokemon or multiple at once. This action cannot be undone!\n\
If you specify more Pokemon than you have, it will release all available.\n\n\
Examples:\n\
  catch-pokemon release pidgey\n\
  catch-pokemon release rattata --number 10\n\
  catch-pokemon release pikachu -n 3")]
    Release {
        /// Name of the Pokemon to release (case insensitive)
        pokemon: String,
        
        /// Number of this Pokemon to release (releases all if you don't have enough)
        #[arg(short = 'n', long, default_value = "1", 
              help = "How many of this Pokemon to release (default: 1)")]
        number: usize,
    },
    
    /// Check if you've caught a specific Pokemon before
    #[command(long_about = "Check your collection status for a specific Pokemon.\n\n\
Two output modes:\n\
- Default: Shows detailed information with catch count and most recent catch\n\
- Boolean: Returns just 'true' or 'false' (useful for scripting)\n\n\
Examples:\n\
  catch-pokemon status charizard\n\
  catch-pokemon status pikachu --boolean\n\
  \n\
Scripting example:\n\
  if [ \"$(catch-pokemon status mewtwo --boolean)\" = \"true\" ]; then\n\
    echo \"You have Mewtwo!\"\n\
  fi")]
    Status {
        /// Name of the Pokemon to check (case insensitive)
        pokemon: String,
        
        /// Output only 'true' or 'false' instead of detailed information
        #[arg(long, help = "Return just true/false for scripting")]
        boolean: bool,
    },
    
    /// Clear your entire Pokemon collection (DESTRUCTIVE)
    #[command(long_about = "Permanently delete all Pokemon from your PC storage.\n\n\
⚠️  WARNING: This action cannot be undone!\n\
All caught Pokemon, catch history, and statistics will be lost.\n\
You will be prompted to confirm before deletion.\n\n\
Example:\n\
  catch-pokemon clear")]
    Clear,

    /// Verify the integrity of your PC storage
    #[command(long_about = "Verify the cryptographic integrity chain of your Pokemon storage.\n\n\
Checks that:\n\
- No Pokemon entries have been added, removed, or reordered\n\
- No entry fields have been tampered with\n\
- The chain is complete from genesis to the latest entry\n\n\
Example:\n\
  catch-pokemon verify")]
    Verify,

    /// Set up shell functions (catch, pc, pokemon_encounter, etc.)
    #[command(long_about = "Install shell functions for the Pokemon catching game.\n\n\
This sets up convenient shell commands:\n\
- catch: Attempt to catch the current wild Pokemon\n\
- pc: View your Pokemon collection\n\
- pokemon_encounter: Generate a new wild Pokemon encounter\n\
- pokemon_new: Force a new encounter\n\
- pokemon_status: Show current Pokemon status\n\
- pokemon_check <name>: Check if you own a specific Pokemon\n\
- pokemon_help: Show all available commands\n\n\
The shell functions are installed to ~/.local/share/catch-pokemon/functions.sh\n\
and automatically sourced from your shell config (.zshrc or .bashrc).\n\n\
Example:\n\
  catch-pokemon setup")]
    Setup,

    /// Update to the latest version
    #[command(long_about = "Download and install the latest version of catch-pokemon.\n\n\
This fetches the latest release from GitHub and replaces the current binary.\n\
Your Pokemon collection and shell functions are preserved.\n\n\
Example:\n\
  catch-pokemon update")]
    Update,

    /// Generate a weighted random Pokemon encounter
    #[command(long_about = "Generate a random Pokemon encounter weighted by rarity.\n\n\
Common Pokemon (high catch rate) appear more often than rare ones.\n\
Legendary and mythical Pokemon are extremely rare encounters.\n\n\
Encounter weights are based on catch rates:\n\
- Common (catch_rate 255): Very frequent\n\
- Uncommon (catch_rate 120): Moderate\n\
- Rare (catch_rate 45-75): Uncommon\n\
- Legendary/Mythical (catch_rate 3): Extremely rare\n\n\
Output modes:\n\
- Default: Prints only the Pokemon name (for scripting)\n\
- --show-pokemon: Also displays the Pokemon sprite\n\n\
Examples:\n\
  catch-pokemon encounter\n\
  catch-pokemon encounter --show-pokemon")]
    Encounter {
        /// Display the Pokemon sprite alongside the name
        #[arg(long, help = "Show the Pokemon sprite using pokemon-colorscripts")]
        show_pokemon: bool,
    },
}

#[derive(Debug, Clone, Copy)]
enum PokeballType {
    Pokeball,
}

impl PokeballType {
    fn catch_modifier(&self) -> f32 {
        match self {
            PokeballType::Pokeball => 1.0,
        }
    }

    fn display_name(&self) -> &str {
        match self {
            PokeballType::Pokeball => "Poké Ball",
        }
    }

    fn ball_symbol(&self) -> String {
        match self {
            PokeballType::Pokeball => "◓".red().to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct PokemonData {
    catch_rate: u8,
    category: String,
    #[serde(default = "default_flee_rate")]
    flee_rate: u8,
    #[serde(default)]
    types: Vec<String>,
    #[serde(default)]
    power_rank: u8,
}

fn default_flee_rate() -> u8 {
    10
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CaughtPokemon {
    name: String,
    caught_at: DateTime<Local>,
    ball_used: String,
    #[serde(default)]
    shiny: bool,
    #[serde(default)]
    prev_hash: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PcStorage {
    pokemon: Vec<CaughtPokemon>,
    #[serde(default)]
    chain_hash: Option<String>,
}

impl PcStorage {
    fn new() -> Self {
        PcStorage { pokemon: Vec::new(), chain_hash: None }
    }
    
    fn load() -> Self {
        let path = get_storage_path();
        if !path.exists() {
            return PcStorage::new();
        }

        // Try to read as encrypted file first
        if let Ok(encrypted_bytes) = fs::read(&path) {
            if let Some(storage) = decrypt_storage(&encrypted_bytes) {
                return storage;
            }
        }

        // Try to read as legacy unencrypted JSON (migration)
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(mut storage) = serde_json::from_str::<PcStorage>(&contents) {
                println!("{}", "Unencrypted PC storage detected. Encrypting...".yellow());
                if storage.chain_hash.is_none() && !storage.pokemon.is_empty() {
                    storage.resign_chain();
                }
                if let Err(e) = storage.save() {
                    eprintln!("Warning: Could not encrypt storage: {}", e);
                } else {
                    println!("{}", "PC storage is now encrypted.".green().bold());
                }
                return storage;
            }
        }

        // IMPORTANT: Do NOT return empty or overwrite — the file exists but we can't decrypt it.
        // This means the binary has a different key than what encrypted the file.
        // Back up the existing file so it's never lost.
        let backup_path = path.with_extension("json.bak");
        if !backup_path.exists() {
            let _ = fs::copy(&path, &backup_path);
            eprintln!("{}", format!("PC storage backed up to {}", backup_path.display()).yellow());
        }

        eprintln!("{}", "Could not decrypt PC storage.".red().bold());
        eprintln!("{}", "This usually means the binary was built with a different key.".red());
        eprintln!("{}", "Your Pokemon data has been backed up and is NOT lost.".yellow());
        eprintln!("To fix: rebuild with the correct BUILD_SECRET_KEY or restore the backup.");
        std::process::exit(1);
    }
    
    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_storage_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let encrypted = encrypt_storage(self)?;
        fs::write(&path, encrypted)?;
        Ok(())
    }
    
    fn add_pokemon(&mut self, name: String, ball: PokeballType, shiny: bool) {
        let key = derive_signing_key();
        let prev_hash = self.chain_hash.clone().unwrap_or_else(|| "genesis".to_string());

        let mut entry = CaughtPokemon {
            name,
            caught_at: Local::now(),
            ball_used: ball.display_name().to_string(),
            shiny,
            prev_hash: Some(prev_hash.clone()),
            signature: None,
        };

        entry.signature = Some(sign_entry(&key, &entry, &prev_hash));
        self.chain_hash = Some(compute_entry_hash(&entry, &prev_hash));
        self.pokemon.push(entry);
    }
    
    fn release_pokemon(&mut self, name: &str, count: usize) -> usize {
        let mut released = 0;

        self.pokemon.retain(|p| {
            if p.name.to_lowercase() == name.to_lowercase() && released < count {
                released += 1;
                false
            } else {
                true
            }
        });

        if released > 0 {
            self.resign_chain();
        }

        released
    }

    fn resign_chain(&mut self) {
        let key = derive_signing_key();
        let mut prev_hash = String::from("genesis");

        for entry in self.pokemon.iter_mut() {
            entry.prev_hash = Some(prev_hash.clone());
            entry.signature = Some(sign_entry(&key, entry, &prev_hash));
            prev_hash = compute_entry_hash(entry, &prev_hash);
        }

        self.chain_hash = if self.pokemon.is_empty() {
            None
        } else {
            Some(prev_hash)
        };
    }
    
    fn has_pokemon(&self, name: &str) -> bool {
        self.pokemon.iter().any(|p| p.name.to_lowercase() == name.to_lowercase())
    }
    
    fn count_pokemon(&self, name: &str) -> usize {
        self.pokemon.iter().filter(|p| p.name.to_lowercase() == name.to_lowercase()).count()
    }
}

fn get_storage_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("catch-pokemon");
    path.push("pc_storage.json");
    path
}

// --- INTEGRITY SYSTEM ---

// Domain separation constants — scattered across the binary to avoid simple extraction
const KDF_DOMAIN: &[u8] = b"catch-pokemon:kdf:v1";
const SIGN_DOMAIN: &[u8] = b"catch-pokemon:sign:v1";
const CHAIN_DOMAIN: &[u8] = b"catch-pokemon:chain:v1";

/// Derive a per-machine signing key using multi-round HMAC-based key stretching.
///
/// The derivation chain:
///   1. HMAC(BUILD_SECRET, domain_separator) → intermediate key
///   2. HMAC(intermediate, hostname:username) → salted key
///   3. 10,000 rounds of HMAC(prev_round, round_counter) → stretched key
///
/// This makes brute-force reversal expensive even if BUILD_SECRET is extracted
/// from the binary, and ties the key to the specific machine + user.
fn derive_signing_key() -> Vec<u8> {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown-host".to_string());
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown-user".to_string());

    // Step 1: Domain-separated intermediate key
    let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&BUILD_SECRET)
        .expect("HMAC accepts any key length");
    mac.update(KDF_DOMAIN);
    let intermediate = mac.finalize().into_bytes();

    // Step 2: Salt with machine identity
    let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&intermediate)
        .expect("HMAC accepts any key length");
    mac.update(format!("{}:{}", hostname, username).as_bytes());
    let mut key = mac.finalize().into_bytes().to_vec();

    // Step 3: Key stretching — 10,000 HMAC rounds
    for round in 0u32..10_000 {
        let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&key)
            .expect("HMAC accepts any key length");
        mac.update(&round.to_le_bytes());
        mac.update(SIGN_DOMAIN);
        key = mac.finalize().into_bytes().to_vec();
    }

    key
}

/// Canonical data string for signing (excludes signature and prev_hash fields)
fn entry_canonical_data(entry: &CaughtPokemon) -> String {
    format!(
        "{}|{}|{}|{}",
        entry.name,
        entry.caught_at.to_rfc3339(),
        entry.ball_used,
        entry.shiny
    )
}

/// Compute the chain hash for an entry (domain-separated)
fn compute_entry_hash(entry: &CaughtPokemon, prev_hash: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(CHAIN_DOMAIN);
    hasher.update(entry_canonical_data(entry).as_bytes());
    hasher.update(prev_hash.as_bytes());
    hex::encode(hasher.finalize())
}

/// HMAC-sign an entry with the derived key
fn sign_entry(key: &[u8], entry: &CaughtPokemon, prev_hash: &str) -> String {
    let mut mac = <HmacSha256 as HmacMac>::new_from_slice(key)
        .expect("HMAC accepts any key length");
    mac.update(entry_canonical_data(entry).as_bytes());
    mac.update(prev_hash.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verify the entire integrity chain. Returns Ok or an error description.
fn verify_chain(storage: &PcStorage) -> Result<(), String> {
    let key = derive_signing_key();
    let mut prev_hash = String::from("genesis");

    for (i, entry) in storage.pokemon.iter().enumerate() {
        let entry_prev = entry.prev_hash.as_deref().unwrap_or("genesis");
        if entry_prev != prev_hash {
            return Err(format!(
                "Chain broken at entry {} ({}): prev_hash mismatch",
                i, entry.name
            ));
        }

        let expected_sig = sign_entry(&key, entry, &prev_hash);
        let actual_sig = entry.signature.as_deref().unwrap_or("");
        if actual_sig != expected_sig {
            return Err(format!(
                "Invalid signature at entry {} ({}). Storage may have been tampered with.",
                i, entry.name
            ));
        }

        prev_hash = compute_entry_hash(entry, &prev_hash);
    }

    if let Some(ref stored_hash) = storage.chain_hash {
        if stored_hash != &prev_hash {
            return Err("Final chain hash mismatch. Entries may have been added or removed.".to_string());
        }
    } else if !storage.pokemon.is_empty() {
        return Err("Missing chain hash on non-empty storage.".to_string());
    }

    Ok(())
}

// --- ENCRYPTION ---
// AES-256-GCM encryption for the entire PC storage file
// The file on disk is an encrypted binary blob — not readable JSON

const ENCRYPTION_DOMAIN: &[u8] = b"catch-pokemon:encryption:v1";

/// Derive a 32-byte AES key from the signing key
fn derive_encryption_key() -> [u8; 32] {
    let signing_key = derive_signing_key();
    let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&signing_key)
        .expect("HMAC accepts any key length");
    mac.update(ENCRYPTION_DOMAIN);
    let result = mac.finalize().into_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypt PC storage: output = [12-byte nonce][AES-256-GCM ciphertext]
fn encrypt_storage(storage: &PcStorage) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key)?;

    let json = serde_json::to_string(storage)?;

    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    for b in nonce_bytes.iter_mut() {
        *b = rng.gen();
    }
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt PC storage from encrypted bytes
fn decrypt_storage(data: &[u8]) -> Option<PcStorage> {
    if data.len() < 13 {
        return None;
    }

    // If it starts with '{', it's unencrypted legacy JSON
    if data[0] == b'{' {
        return None;
    }

    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;

    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
    let json_str = String::from_utf8(plaintext).ok()?;
    serde_json::from_str(&json_str).ok()
}

// Embed the art files directly in the binary
const POKEBALL_STILL: &str = include_str!("../static/art/pokeball-still.txt");
const POKEBALL_LEFT: &str = include_str!("../static/art/pokeball-left.txt");
const POKEBALL_RIGHT: &str = include_str!("../static/art/pokeball-right.txt");
const POKEBALL_CAUGHT: &str = include_str!("../static/art/pokeball-caught.txt");
const POKEBALL_NOT_CAUGHT: &str = include_str!("../static/art/pokeball-not-caught.txt");

// Embed the Pokemon data directly in the binary
const POKEMON_DATA: &str = include_str!("../data/pokemon.json");

// Embed valid pokemon-colorscripts names (for encounter filtering)
const VALID_POKEMON: &str = include_str!("../data/valid_pokemon.txt");

// Embed the shell functions directly in the binary
const SHELL_FUNCTIONS: &str = include_str!("../shell/functions.sh");

fn load_pokeball_art(art_type: &str) -> Vec<String> {
    let content = match art_type {
        "still" => POKEBALL_STILL,
        "left" => POKEBALL_LEFT,
        "right" => POKEBALL_RIGHT,
        "caught" => POKEBALL_CAUGHT,
        "not-caught" => POKEBALL_NOT_CAUGHT,
        _ => return vec![],
    };
    
    content.lines().map(|line| line.to_string()).collect()
}

fn clear_lines(count: usize) {
    for _ in 0..count {
        stdout().execute(cursor::MoveUp(1)).unwrap();
        stdout().execute(terminal::Clear(terminal::ClearType::CurrentLine)).unwrap();
    }
    stdout().flush().unwrap();
}

fn display_pokeball_art(lines: &[String]) {
    for line in lines {
        println!("{}", line);
    }
    stdout().flush().unwrap();
}

fn get_pokemon_catch_rate(pokemon_name: &str) -> u8 {
    // Parse the embedded Pokemon data once
    let pokemon_db: HashMap<String, PokemonData> = match serde_json::from_str(POKEMON_DATA) {
        Ok(data) => data,
        Err(_) => return 120, // Default catch rate if JSON parsing fails
    };
    
    // Normalize the Pokemon name to match our data format
    let normalized_name = pokemon_name.to_lowercase()
        .replace("'", "")
        .replace(".", "")
        .replace(" ", "_")
        .replace("-", "_");
    
    // Look up the Pokemon in our database
    match pokemon_db.get(&normalized_name) {
        Some(data) => data.catch_rate,
        None => 120, // Default catch rate for unknown Pokemon
    }
}

/// Flee rate read from pokemon.json — rarer Pokemon are more likely to run
fn get_flee_rate(pokemon_name: &str) -> f32 {
    let pokemon_db: HashMap<String, PokemonData> = match serde_json::from_str(POKEMON_DATA) {
        Ok(data) => data,
        Err(_) => return 10.0,
    };

    let normalized_name = pokemon_name.to_lowercase()
        .replace("'", "")
        .replace(".", "")
        .replace(" ", "_")
        .replace("-", "_");

    match pokemon_db.get(&normalized_name) {
        Some(data) => data.flee_rate as f32,
        None => 10.0,
    }
}

fn calculate_catch_chance(pokemon_name: &str, ball: PokeballType) -> f32 {
    let base_catch_rate = get_pokemon_catch_rate(pokemon_name) as f32;
    let ball_modifier = ball.catch_modifier();
    
    let modified_rate = (base_catch_rate * ball_modifier).min(255.0);
    
    (modified_rate / 255.0 * 100.0).min(100.0)
}

fn throw_pokeball_animation(ball: PokeballType) {
    println!("You throw a {}!", ball.display_name());
    thread::sleep(Duration::from_millis(300));
}

fn wiggle_animation(wiggle_num: u8, ball: PokeballType, caught: bool) {
    let still_art = load_pokeball_art("still");
    let left_art = load_pokeball_art("left");
    let right_art = load_pokeball_art("right");
    
    if still_art.is_empty() || left_art.is_empty() || right_art.is_empty() {
        // Fallback to simple animation if art files can't be loaded
        let ball_symbol = ball.ball_symbol();
        println!();
        print!("   {}   ", ball_symbol);
        for _ in 1..=wiggle_num {
            print!(".");
            stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(400));
        }
        thread::sleep(Duration::from_millis(500));
        return;
    }
    
    let art_height = still_art.len();
    
    // Display initial still pokeball
    println!();
    display_pokeball_art(&still_art);
    thread::sleep(Duration::from_millis(500));
    
    // Perform shaking animation for each wiggle
    for i in 1..=wiggle_num {
        // Shake left
        clear_lines(art_height);
        display_pokeball_art(&left_art);
        thread::sleep(Duration::from_millis(150));
        
        // Shake right
        clear_lines(art_height);
        display_pokeball_art(&right_art);
        thread::sleep(Duration::from_millis(150));
        
        // Shake left again
        clear_lines(art_height);
        display_pokeball_art(&left_art);
        thread::sleep(Duration::from_millis(150));
        
        // Back to center
        clear_lines(art_height);
        display_pokeball_art(&still_art);
        
        // Pause between wiggles, longer pause for dramatic effect
        if i < wiggle_num {
            thread::sleep(Duration::from_millis(600));
        } else {
            thread::sleep(Duration::from_millis(800));
        }
    }
    
    // Final result animation
    if caught {
        // Load and display caught animation
        let caught_art = load_pokeball_art("caught");
        if !caught_art.is_empty() {
            clear_lines(art_height);
            display_pokeball_art(&caught_art);
            thread::sleep(Duration::from_millis(1000));
        }
    } else {
        // Load and display escape animation (pokeball opens)
        let not_caught_art = load_pokeball_art("not-caught");
        if !not_caught_art.is_empty() {
            clear_lines(art_height);
            display_pokeball_art(&not_caught_art);
            thread::sleep(Duration::from_millis(1000));
        }
    }
}


fn catch_pokemon(pokemon: String, skip_animation: bool, hide_pokemon: bool, shiny: bool, attempt: u32) {
    let ball = PokeballType::Pokeball;
    let catch_chance = calculate_catch_chance(&pokemon, ball);
    
    if !hide_pokemon {
        println!();
        println!("A wild {} appeared!", pokemon.green().bold());
        
        let output = Command::new("pokemon-colorscripts")
            .args(&["-n", &pokemon, "--no-title"])
            .output();
        
        if let Ok(result) = output {
            if result.status.success() {
                print!("{}", String::from_utf8_lossy(&result.stdout));
            }
        }
    }
    
    println!();
    println!(
        "{}",
        format!("Throwing {} at {}!", ball.display_name(), pokemon).cyan().bold()
    );
    println!("Catch chance: {}", format!("{:.1}%", catch_chance).bright_yellow().bold());
    println!();

    let mut rng = rand::thread_rng();
    let catch_roll = rng.gen_range(0.0..100.0);
    let caught = catch_roll < catch_chance;

    if !skip_animation {
        throw_pokeball_animation(ball);

        let wiggles = if catch_chance > 90.0 {
            2
        } else if catch_chance > 50.0 {
            3
        } else {
            4
        };
        
        // Single wiggle animation that handles all wiggles
        wiggle_animation(wiggles, ball, caught);
    }

    // Clear the animation completely
    print!("\r{}\r", " ".repeat(100));
    stdout().flush().unwrap();
    println!();

    if caught {
        println!();
        if shiny {
            println!(
                "{}",
                format!("Gotcha! A shiny {} was caught!", pokemon)
                    .yellow()
                    .bold()
            );
        } else {
            println!(
                "{}",
                format!("Gotcha! {} was caught!", pokemon)
                    .green()
                    .bold()
            );
        }
        println!();

        let mut storage = PcStorage::load();
        storage.add_pokemon(pokemon.clone(), ball, shiny);
        if let Err(e) = storage.save() {
            eprintln!("Warning: Could not save to PC: {}", e);
        } else {
            println!();
            if shiny {
                println!("{}", format!("A shiny {} has been sent to your PC!", pokemon).yellow().bold());
            } else {
                println!("{} has been sent to your PC!", pokemon.cyan());
            }
        }

    } else {
        // Rolling flee rate: base + 5% per additional attempt, capped at 80%
        let base_flee = get_flee_rate(&pokemon);
        let flee_bonus = (attempt.saturating_sub(1) as f32) * 5.0;
        let flee_rate = (base_flee + flee_bonus).min(80.0);
        let run_away_chance = rng.gen_range(0.0..100.0);
        if run_away_chance < flee_rate {
            println!(
                "{}",
                format!("Oh no! The wild {} broke free and ran away!", pokemon).red()
            );
        } else {
            println!(
                "{}",
                format!("Oh no! The wild {} broke free!", pokemon).red()
            );
            
            // Show what the Pokemon is doing after breaking free
            let actions = [
                "makes a face at you",
                "sticks its tongue out",
                "laughs mockingly",
                "does a little dance",
                "shakes its head disapprovingly",
                "crosses its arms defiantly",
                "winks at you cheekily",
                "spins around showing off",
                &format!("shouts \"{}!\" loudly", pokemon.to_uppercase()),
                "gives you a smug look"
            ];
            
            let action = actions[rng.gen_range(0..actions.len())];
            println!("{} {}.", pokemon, action);
        }
    }
}

fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let text = text.to_lowercase();
    let pattern = pattern.to_lowercase();
    
    // Simple fuzzy matching: check if all characters in pattern appear in order in text
    let mut pattern_chars = pattern.chars();
    let mut current_char = pattern_chars.next();
    
    for text_char in text.chars() {
        if let Some(pattern_char) = current_char {
            if text_char == pattern_char {
                current_char = pattern_chars.next();
            }
        }
    }
    
    current_char.is_none()
}

fn interactive_pokemon_search(storage: &PcStorage) -> Result<(), Box<dyn std::error::Error>> {
    // Get unique Pokemon names
    let mut pokemon_names: Vec<String> = storage.pokemon
        .iter()
        .map(|p| p.name.clone())
        .collect();
    pokemon_names.sort();
    pokemon_names.dedup();
    
    let mut search_term = String::new();
    let mut selected_index = 0;
    
    // Enable raw mode for direct input handling
    terminal::enable_raw_mode()?;
    
    // Hide cursor for cleaner display
    print!("\x1B[?25l");
    stdout().flush()?;
    
    loop {
        // Clear screen completely
        print!("\x1B[2J\x1B[1;1H");
        
        // Filter Pokemon based on current search term
        let filtered: Vec<&String> = if search_term.is_empty() {
            pokemon_names.iter().collect()
        } else {
            pokemon_names
                .iter()
                .filter(|name| fuzzy_match(name, &search_term))
                .collect()
        };
        
        // Update selected index if it's out of bounds
        if selected_index >= filtered.len() && !filtered.is_empty() {
            selected_index = filtered.len() - 1;
        }
        
        // Display search line with highlighting
        print!("Search: ");
        print!("\x1B[33m{}\x1B[0m", search_term); // Yellow search term
        print!("\x1B[90m_\x1B[0m"); // Gray cursor
        println!();
        
        if !filtered.is_empty() {
            println!("\x1B[36m{}/{}\x1B[0m", selected_index + 1, filtered.len()); // Cyan counter
        } else {
            println!("\x1B[31m0/0\x1B[0m"); // Red when no results
        }
        println!(); // Empty line
        
        // Display results with clear highlighting
        let display_count = 8.min(filtered.len());
        for (i, pokemon_name) in filtered.iter().take(display_count).enumerate() {
            if i == selected_index {
                // Bright highlighted selection with background
                print!("\x1B[1;37;44m"); // Bold white text on blue background
                println!(" ► {} ", pokemon_name);
                print!("\x1B[0m"); // Reset
            } else {
                // Regular white text
                println!("\x1B[37m   {}\x1B[0m", pokemon_name);
            }
        }
        
        if filtered.len() > display_count {
            println!("\x1B[90m   ... {} more\x1B[0m", filtered.len() - display_count); // Gray
        }
        
        stdout().flush()?;
        
        // Handle input
        if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
            match code {
                KeyCode::Esc => break,
                KeyCode::Enter => {
                    if !filtered.is_empty() && selected_index < filtered.len() {
                        let selected_pokemon = filtered[selected_index];
                        
                        // Show cursor and disable raw mode
                        print!("\x1B[?25h");
                        terminal::disable_raw_mode()?;
                        
                        // Clear screen and show details
                        print!("\x1B[2J\x1B[1;1H");
                        stdout().flush()?;
                        
                        // Get ball counts for this Pokemon
                        let mut ball_counts: HashMap<String, usize> = HashMap::new();
                        for p in &storage.pokemon {
                            if p.name == *selected_pokemon {
                                *ball_counts.entry(p.ball_used.clone()).or_insert(0) += 1;
                            }
                        }
                        
                        show_pokemon_details(selected_pokemon, &ball_counts, storage);
                        return Ok(());
                    }
                }
                KeyCode::Up => {
                    if !filtered.is_empty() && selected_index > 0 {
                        selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() && selected_index < filtered.len() - 1 {
                        selected_index += 1;
                    }
                }
                KeyCode::Backspace => {
                    if search_term.pop().is_some() {
                        selected_index = 0;
                    }
                }
                KeyCode::Char(c) => {
                    if modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                        break;
                    } else {
                        search_term.push(c);
                        selected_index = 0;
                    }
                }
                _ => {}
            }
        }
    }
    
    // Restore cursor and disable raw mode
    print!("\x1B[?25h");
    terminal::disable_raw_mode()?;
    print!("\x1B[2J\x1B[1;1H");
    println!("Search cancelled.");
    Ok(())
}

fn show_pokemon_details(pokemon_name: &str, ball_counts: &HashMap<String, usize>, storage: &PcStorage) {
    let total_count: usize = ball_counts.values().sum();
    
    println!();
    println!("{}", format!("=== {} ===", pokemon_name).green().bold());
    println!();
    
    // Show Pokemon sprite using pokemon-colorscripts
    let output = Command::new("pokemon-colorscripts")
        .args(&["-n", pokemon_name, "--no-title"])
        .output();
    
    if let Ok(result) = output {
        if result.status.success() {
            print!("{}", String::from_utf8_lossy(&result.stdout));
        }
    }
    
    println!();
    println!("{}", format!("Total caught: {}", total_count).cyan().bold());
    println!();
    
    // Show breakdown by ball type
    println!("Caught with:");
    for (ball, count) in ball_counts {
        if *count == 1 {
            println!("  • {} with {}", "1".yellow(), ball.magenta());
        } else {
            println!("  • {} with {}", count.to_string().yellow(), ball.magenta());
        }
    }
    
    println!();
    
    // Show catch history for this Pokemon
    let pokemon_catches: Vec<_> = storage.pokemon
        .iter()
        .filter(|p| p.name.to_lowercase() == pokemon_name.to_lowercase())
        .collect();
    
    println!("Catch history:");
    for (i, pokemon) in pokemon_catches.iter().rev().enumerate() {
        if i >= 5 { break; } // Show only last 5 catches
        println!("  • {} at {}", 
                pokemon.ball_used.cyan(),
                pokemon.caught_at.format("%Y-%m-%d %H:%M"));
    }
    
    if pokemon_catches.len() > 5 {
        println!("  ... and {} more", pokemon_catches.len() - 5);
    }
}

fn show_pc(search: bool) {
    let storage = PcStorage::load();

    if storage.pokemon.is_empty() {
        println!("{}", "Your PC is empty. Go catch some Pokemon!".yellow());
        return;
    }

    // Verify integrity before displaying
    if storage.chain_hash.is_some() {
        if let Err(msg) = verify_chain(&storage) {
            println!("{}", format!("PC integrity check FAILED: {}", msg).red().bold());
            println!("{}", "Your PC storage appears to have been tampered with.".red());
            println!("Run 'catch-pokemon verify' for details.");
            return;
        }
    }
    
    // If search flag is provided, launch interactive search
    if search {
        if let Err(e) = interactive_pokemon_search(&storage) {
            eprintln!("Error in interactive search: {}", e);
        }
        return;
    }

    // Load Pokemon database for type/power info
    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();

    println!();
    println!("{}", "  Pokemon PC Storage".cyan().bold());
    println!("{}", "  ══════════════════".cyan());
    println!();

    // Group Pokemon by name with counts
    let mut pokemon_counts: HashMap<String, usize> = HashMap::new();
    let mut pokemon_shiny_counts: HashMap<String, usize> = HashMap::new();
    for p in &storage.pokemon {
        *pokemon_counts.entry(p.name.clone()).or_insert(0) += 1;
        if p.shiny {
            *pokemon_shiny_counts.entry(p.name.clone()).or_insert(0) += 1;
        }
    }

    let mut sorted_names: Vec<_> = pokemon_counts.keys().collect();
    sorted_names.sort();

    for name in &sorted_names {
        let count = pokemon_counts[*name];
        let shiny_count = pokemon_shiny_counts.get(*name).copied().unwrap_or(0);

        // Look up type and power from database
        let normalized = name.replace("-", "_");
        let (types_str, power, category) = if let Some(data) = pokemon_db.get(&normalized) {
            let type_strings: Vec<String> = data.types.iter().map(|t| {
                match t.as_str() {
                    "fire"     => t.red().bold().to_string(),
                    "water"    => t.blue().bold().to_string(),
                    "grass"    => t.green().bold().to_string(),
                    "electric" => t.yellow().bold().to_string(),
                    "ice"      => t.cyan().bold().to_string(),
                    "fighting" => t.red().to_string(),
                    "poison"   => t.purple().to_string(),
                    "ground"   => t.yellow().to_string(),
                    "flying"   => t.cyan().to_string(),
                    "psychic"  => t.magenta().bold().to_string(),
                    "bug"      => t.green().to_string(),
                    "rock"     => t.yellow().dimmed().to_string(),
                    "ghost"    => t.purple().bold().to_string(),
                    "dragon"   => t.blue().bold().to_string(),
                    "dark"     => t.white().dimmed().to_string(),
                    "steel"    => t.white().to_string(),
                    "fairy"    => t.magenta().to_string(),
                    "normal"   => t.white().to_string(),
                    _          => t.to_string(),
                }
            }).collect();
            (type_strings.join(" / "), data.power_rank, data.category.clone())
        } else {
            ("???".to_string(), 0, "unknown".to_string())
        };

        // Format category color
        let cat_display = match category.as_str() {
            "legendary"        => "Legendary".red().bold().to_string(),
            "mythical"         => "Mythical".magenta().bold().to_string(),
            "pseudo_legendary" => "Pseudo-Legendary".yellow().bold().to_string(),
            "starter"          => "Starter".green().bold().to_string(),
            "starter_evolution" => "Starter Evo".green().to_string(),
            "rare"             => "Rare".cyan().bold().to_string(),
            "baby"             => "Baby".bright_magenta().to_string(),
            "uncommon"         => "Uncommon".white().to_string(),
            "common"           => "Common".bright_black().to_string(),
            _                  => category.clone(),
        };

        // Build display line
        let count_str = if count > 1 {
            format!(" x{}", count).yellow().to_string()
        } else {
            String::new()
        };

        let shiny_str = if shiny_count > 0 {
            format!(" ({} shiny)", shiny_count).yellow().bold().to_string()
        } else {
            String::new()
        };

        println!("  {}{}{}", name.green().bold(), count_str, shiny_str);
        println!("    {} | Power: {} | {}",
            types_str,
            format!("{}", power).bright_yellow().bold(),
            cat_display,
        );
    }

    println!();
    println!("  {}", "──────────────────────────────────────".dimmed());
    println!();
    println!("  {}", format!("Total: {} Pokemon | {} unique",
        storage.pokemon.len(),
        sorted_names.len()
    ).yellow().bold());

    let total_shiny: usize = pokemon_shiny_counts.values().sum();
    if total_shiny > 0 {
        println!("  {}", format!("Shinies: {}", total_shiny).yellow());
    }

    println!();
    println!("  {}", "──────────────────────────────────────".dimmed());
    println!();
    println!("  {}", "Recent catches:".dimmed());
    for pokemon in storage.pokemon.iter().rev().take(5) {
        let shiny_tag = if pokemon.shiny { " [Shiny]".yellow().to_string() } else { String::new() };
        println!("    {} at {}{}",
                pokemon.name.green(),
                pokemon.caught_at.format("%Y-%m-%d %H:%M").to_string().dimmed(),
                shiny_tag);
    }
}

fn release_pokemon(pokemon_name: String, number: usize) {
    let mut storage = PcStorage::load();
    
    if storage.pokemon.is_empty() {
        println!("{}", "Your PC is empty. No Pokemon to release!".yellow());
        return;
    }
    
    let available_count = storage.count_pokemon(&pokemon_name);
    if available_count == 0 {
        println!("{}", format!("You don't have any {} in your PC.", pokemon_name).red());
        return;
    }
    
    let to_release = number.min(available_count);
    if number > available_count {
        println!("{}", format!("You only have {} {} in your PC, releasing all of them.", 
                 available_count, pokemon_name).yellow());
    }
    
    println!("{}", 
             format!("Are you sure you want to release {} {}{}? This cannot be undone!", 
                     to_release, pokemon_name, if to_release > 1 { "s" } else { "" }).red().bold());
    print!("Type 'yes' to confirm: ");
    stdout().flush().unwrap();
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    if input.trim().to_lowercase() == "yes" {
        let released = storage.release_pokemon(&pokemon_name, to_release);
        
        if let Err(e) = storage.save() {
            eprintln!("Warning: Could not save to PC: {}", e);
        } else {
            println!();
            println!("{}", 
                     format!("Released {} {}{}! They've returned to the wild.", 
                             released, pokemon_name, if released > 1 { "s" } else { "" }).green().bold());
            
            if storage.count_pokemon(&pokemon_name) > 0 {
                println!("You still have {} {} remaining in your PC.", 
                        storage.count_pokemon(&pokemon_name), pokemon_name);
            }
        }
    } else {
        println!("Release cancelled.");
    }
}

fn check_pokemon(pokemon_name: String, boolean_mode: bool) {
    let storage = PcStorage::load();
    
    if boolean_mode {
        // Just return true or false
        println!("{}", storage.has_pokemon(&pokemon_name));
        return;
    }
    
    if storage.has_pokemon(&pokemon_name) {
        let count = storage.count_pokemon(&pokemon_name);
        println!("{}", 
                format!("✅ You have caught {} before! You have {} in your PC.", 
                        pokemon_name, 
                        if count == 1 { "1".to_string() } else { count.to_string() }).green().bold());
        
        // Show most recent catch
        if let Some(most_recent) = storage.pokemon.iter()
            .filter(|p| p.name.to_lowercase() == pokemon_name.to_lowercase())
            .max_by_key(|p| p.caught_at) {
            println!("Most recent catch: {} with {} at {}", 
                    most_recent.name.cyan(),
                    most_recent.ball_used.magenta(),
                    most_recent.caught_at.format("%Y-%m-%d %H:%M"));
        }
    } else {
        println!("{}", 
                format!("❌ You haven't caught {} yet. Go catch one!", pokemon_name).red());
    }
}

fn encounter_pokemon(show_pokemon: bool) {
    // Parse the Pokemon database
    let pokemon_db: HashMap<String, PokemonData> = match serde_json::from_str(POKEMON_DATA) {
        Ok(data) => data,
        Err(_) => {
            eprintln!("{}", "Error: Could not load Pokemon database.".red());
            return;
        }
    };

    // Load valid pokemon-colorscripts names and filter to only encounterable Pokemon
    let valid_names: std::collections::HashSet<&str> = VALID_POKEMON
        .lines()
        .filter(|l| !l.is_empty())
        .collect();

    // Build weighted list: only include Pokemon that pokemon-colorscripts supports
    let pokemon_list: Vec<(&String, &PokemonData)> = pokemon_db
        .iter()
        .filter(|(name, _)| valid_names.contains(name.as_str()))
        .collect();
    let total_weight: u32 = pokemon_list.iter().map(|(_, data)| data.catch_rate as u32).sum();

    let mut rng = rand::thread_rng();
    let roll = rng.gen_range(0..total_weight);

    let mut cumulative: u32 = 0;
    let mut chosen_name = "";
    for (name, data) in &pokemon_list {
        cumulative += data.catch_rate as u32;
        if roll < cumulative {
            chosen_name = name;
            break;
        }
    }

    // Convert internal name format back to display format (underscores to hyphens for pokemon-colorscripts)
    let display_name = chosen_name.replace('_', "-");

    // 3% chance of shiny encounter
    // 1% chance of shiny encounter
    let is_shiny = rng.gen_range(0.0..100.0) < 1.0;

    // Always print the name (for scripting use)
    println!("{}", display_name);

    // Print shiny status on second line (for scripting use)
    println!("Shiny: {}", is_shiny);

    if show_pokemon {
        let mut args = vec!["-n", &display_name, "--no-title"];
        if is_shiny {
            args.push("-s");
        }

        let output = Command::new("pokemon-colorscripts")
            .args(&args)
            .output();

        if let Ok(result) = output {
            if result.status.success() {
                print!("{}", String::from_utf8_lossy(&result.stdout));
            }
        }

        // Show category and catch info
        if let Some(data) = pokemon_db.get(chosen_name) {
            let category_display = match data.category.as_str() {
                "legendary" => format!("Legendary").red().bold().to_string(),
                "mythical" => format!("Mythical").magenta().bold().to_string(),
                "pseudo_legendary" => format!("Pseudo-Legendary").yellow().bold().to_string(),
                "starter" => format!("Starter").green().bold().to_string(),
                "starter_evolution" => format!("Starter Evolution").green().to_string(),
                "rare" => format!("Rare").cyan().bold().to_string(),
                "baby" => format!("Baby").bright_magenta().to_string(),
                "uncommon" => format!("Uncommon").white().to_string(),
                "common" => format!("Common").bright_black().to_string(),
                other => other.to_string(),
            };
            println!("Category: {}", category_display);

            // Display types with color coding
            if !data.types.is_empty() {
                let type_strings: Vec<String> = data.types.iter().map(|t| {
                    match t.as_str() {
                        "fire"     => format!("{}", t.red().bold()),
                        "water"    => format!("{}", t.blue().bold()),
                        "grass"    => format!("{}", t.green().bold()),
                        "electric" => format!("{}", t.yellow().bold()),
                        "ice"      => format!("{}", t.cyan().bold()),
                        "fighting" => format!("{}", t.red()),
                        "poison"   => format!("{}", t.purple()),
                        "ground"   => format!("{}", t.yellow()),
                        "flying"   => format!("{}", t.cyan()),
                        "psychic"  => format!("{}", t.magenta().bold()),
                        "bug"      => format!("{}", t.green()),
                        "rock"     => format!("{}", t.yellow().dimmed()),
                        "ghost"    => format!("{}", t.purple().bold()),
                        "dragon"   => format!("{}", t.blue().bold()),
                        "dark"     => format!("{}", t.white().dimmed()),
                        "steel"    => format!("{}", t.white()),
                        "fairy"    => format!("{}", t.magenta()),
                        "normal"   => format!("{}", t.white()),
                        _          => t.to_string(),
                    }
                }).collect();
                println!("Type: {}", type_strings.join(" / "));
            }

            let catch_pct = data.catch_rate as f32 / 255.0 * 100.0;
            println!("Base catch rate: {}", format!("{:.1}%", catch_pct).bright_yellow().bold());
        }
    }
}

fn verify_pc() {
    // Use PcStorage::load which handles decryption
    let storage = PcStorage::load();

    if storage.pokemon.is_empty() {
        println!("{}", "PC is empty. Nothing to verify.".yellow());
        return;
    }

    if storage.chain_hash.is_none() {
        println!("{}", "Storage is unsigned (legacy format).".yellow());
        return;
    }

    match verify_chain(&storage) {
        Ok(()) => {
            println!("{}", format!(
                "Integrity check PASSED. All {} entries verified.",
                storage.pokemon.len()
            ).green().bold());
        }
        Err(msg) => {
            println!("{}", format!("Integrity check FAILED: {}", msg).red().bold());
            std::process::exit(1);
        }
    }
}

fn setup_shell() {
    // Determine install directory
    let mut functions_dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    functions_dir.push("catch-pokemon");

    // Create directory
    if let Err(e) = fs::create_dir_all(&functions_dir) {
        eprintln!("{}", format!("Error creating directory: {}", e).red());
        return;
    }

    // Write shell functions
    let functions_path = functions_dir.join("functions.sh");
    if let Err(e) = fs::write(&functions_path, SHELL_FUNCTIONS) {
        eprintln!("{}", format!("Error writing shell functions: {}", e).red());
        return;
    }
    println!("{}", format!("Shell functions installed to {}", functions_path.display()).green());

    // Detect shell config
    let shell = std::env::var("SHELL").unwrap_or_default();
    let shell_config = if shell.ends_with("zsh") {
        dirs::home_dir().map(|h| h.join(".zshrc"))
    } else if shell.ends_with("bash") {
        dirs::home_dir().map(|h| h.join(".bashrc"))
    } else {
        dirs::home_dir().map(|h| h.join(".profile"))
    };

    let Some(config_path) = shell_config else {
        eprintln!("{}", "Could not determine shell config path.".yellow());
        println!("Add this line to your shell config manually:");
        println!("  source \"{}\"", functions_path.display());
        return;
    };

    // Check if already configured
    let source_line = format!("source \"{}\"", functions_path.display());
    if let Ok(contents) = fs::read_to_string(&config_path) {
        if contents.contains("catch-pokemon/functions.sh") {
            println!("{}", format!("Shell config already configured in {}", config_path.display()).green());
            println!();
            println!("{}", "Setup complete! Restart your terminal or run:".cyan().bold());
            println!("  source {}", config_path.display());
            return;
        }
    }

    // Append source line
    let addition = format!("\n# catch-pokemon shell functions (catch, pc, pokemon_encounter, etc.)\n{}\n", source_line);
    if let Err(e) = fs::OpenOptions::new().append(true).open(&config_path).and_then(|mut f| {
        use std::io::Write;
        f.write_all(addition.as_bytes())
    }) {
        eprintln!("{}", format!("Error updating {}: {}", config_path.display(), e).red());
        println!("Add this line manually:");
        println!("  {}", source_line);
        return;
    }

    println!("{}", format!("Added to {}", config_path.display()).green());
    println!();
    println!("{}", "Setup complete! Restart your terminal or run:".cyan().bold());
    println!("  source {}", config_path.display());
}

fn clear_pc() {
    println!("{}", "Are you sure you want to clear your PC? This cannot be undone!".red().bold());
    print!("Type 'yes' to confirm: ");
    stdout().flush().unwrap();
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    if input.trim().to_lowercase() == "yes" {
        let path = get_storage_path();
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                eprintln!("Error clearing PC: {}", e);
            } else {
                println!("{}", "PC storage cleared!".green());
            }
        } else {
            println!("PC was already empty.");
        }
    } else {
        println!("Clear cancelled.");
    }
}

fn update_binary() {
    println!("{}", "Checking for updates...".cyan());

    let status = Command::new("sh")
        .arg("-c")
        .arg("curl -sSL https://raw.githubusercontent.com/matthewmyrick/catch-pokemon/main/install.sh | bash")
        .status();

    match status {
        Ok(s) if s.success() => {
            println!();
            println!("{}", "Update complete! Restart your terminal to use the new version.".green().bold());
        }
        _ => {
            eprintln!("{}", "Update failed. Check your internet connection.".red());
        }
    }
}

fn main() {
    let args = Args::parse();
    
    match args.command {
        Commands::Catch { pokemon, skip_animation, hide_pokemon, shiny, attempt } => {
            catch_pokemon(pokemon, skip_animation, hide_pokemon, shiny, attempt);
        },
        Commands::Pc { search } => {
            show_pc(search);
        },
        Commands::Release { pokemon, number } => {
            release_pokemon(pokemon, number);
        },
        Commands::Status { pokemon, boolean } => {
            check_pokemon(pokemon, boolean);
        },
        Commands::Clear => {
            clear_pc();
        },
        Commands::Verify => {
            verify_pc();
        },
        Commands::Setup => {
            setup_shell();
        },
        Commands::Encounter { show_pokemon } => {
            encounter_pokemon(show_pokemon);
        },
        Commands::Update => {
            update_binary();
        }
    }
}