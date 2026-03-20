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
        /// Name of the Pokemon to catch
        #[arg(hide = true)]
        pokemon: String,

        /// Skip the animated Pokeball throwing sequence
        #[arg(short = 's', long, help = "Skip animations for faster catching", hide = true)]
        skip_animation: bool,

        /// Hide the Pokemon ASCII art when it appears
        #[arg(long, default_value = "false", hide = true)]
        hide_pokemon: bool,

        /// Mark this Pokemon as shiny (set by encounter system)
        #[arg(long, default_value = "false", hide = true)]
        shiny: bool,

        /// Session token from encounter (required to prevent manual catching)
        #[arg(long, hide = true)]
        token: Option<String>,

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

    /// Restore PC from a plaintext backup JSON file
    #[command(long_about = "Restore your Pokemon collection from a plaintext backup JSON file.\n\n\
This re-encrypts and re-signs the data with the current binary's key.\n\
Use this if your PC got corrupted or you're migrating to a new machine.\n\n\
The backup file is pc_backup.json in your storage directory, or you can\n\
provide a path to any backup file.\n\n\
Example:\n\
  catch-pokemon restore\n\
  catch-pokemon restore --file /path/to/pc_backup.json")]
    Restore {
        /// Path to the plaintext backup JSON file
        #[arg(long)]
        file: Option<String>,
    },

    /// Browse the Pokedex — see all Pokemon, track what you've seen and caught
    #[command(long_about = "Browse the full Pokedex with an interactive TUI.\n\n\
Shows all Pokemon with their types, power rank, and catch status.\n\
Pokemon you've encountered are marked as seen, caught ones are marked differently.\n\
Use fuzzy search to filter by name.\n\n\
Example:\n\
  catch-pokemon pokedex")]
    Pokedex,

    /// Manage your battle team (up to 20 Pokemon)
    #[command(long_about = "Manage your battle team for online battles.\n\n\
Your battle team holds up to 20 Pokemon selected from your PC.\n\
This is the roster you bring to battles — opponents see your battle team,\n\
and you pick 6 from it each round.\n\n\
The battle team is stored in an encrypted file alongside your PC.\n\n\
Examples:\n\
  catch-pokemon team                    # View your battle team\n\
  catch-pokemon team --add pikachu      # Add a Pokemon from your PC\n\
  catch-pokemon team --remove pikachu   # Remove a Pokemon from your team\n\
  catch-pokemon team --clear            # Clear the entire team")]
    Team {
        /// Add a Pokemon from your PC to the battle team
        #[arg(long)]
        add: Option<String>,

        /// Remove a Pokemon from the battle team
        #[arg(long)]
        remove: Option<String>,

        /// Clear the entire battle team
        #[arg(long)]
        clear: bool,
    },

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

        // Write plaintext backup for recovery
        let backup_path = path.with_file_name("pc_backup.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&backup_path, json)?;

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

fn get_pokedex_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("catch-pokemon");
    path.push("pokedex.json");
    path
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PokedexEntry {
    name: String,
    seen: bool,
    caught: bool,
    seen_at: Option<DateTime<Local>>,
    caught_at: Option<DateTime<Local>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Pokedex {
    entries: HashMap<String, PokedexEntry>,
}

impl Pokedex {
    fn new() -> Self {
        Pokedex { entries: HashMap::new() }
    }

    fn load() -> Self {
        let path = get_pokedex_path();
        if !path.exists() {
            return Pokedex::new();
        }
        if let Ok(data) = fs::read(&path) {
            if let Some(dex) = decrypt_pokedex(&data) {
                return dex;
            }
        }
        // Don't wipe — back up
        if path.exists() {
            let backup = path.with_extension("json.bak");
            if !backup.exists() {
                let _ = fs::copy(&path, &backup);
            }
            eprintln!("{}", "Could not decrypt Pokedex. Backed up.".red());
            eprintln!("{}", "Starting fresh Pokedex.".yellow());
        }
        Pokedex::new()
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_pokedex_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encrypted = encrypt_pokedex(self)?;
        fs::write(&path, encrypted)?;

        // Plaintext backup for recovery
        let backup_path = path.with_file_name("pokedex_backup.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&backup_path, json)?;
        Ok(())
    }

    fn mark_seen(&mut self, name: &str) {
        let normalized = name.to_lowercase();
        let entry = self.entries.entry(normalized.clone()).or_insert_with(|| PokedexEntry {
            name: normalized,
            seen: false,
            caught: false,
            seen_at: None,
            caught_at: None,
        });
        if !entry.seen {
            entry.seen = true;
            entry.seen_at = Some(Local::now());
        }
    }

    fn mark_caught(&mut self, name: &str) {
        let normalized = name.to_lowercase();
        let entry = self.entries.entry(normalized.clone()).or_insert_with(|| PokedexEntry {
            name: normalized,
            seen: false,
            caught: false,
            seen_at: None,
            caught_at: None,
        });
        if !entry.seen {
            entry.seen = true;
            entry.seen_at = Some(Local::now());
        }
        if !entry.caught {
            entry.caught = true;
            entry.caught_at = Some(Local::now());
        }
    }
}

fn encrypt_pokedex(dex: &Pokedex) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let json = serde_json::to_string(dex)?;
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    for b in nonce_bytes.iter_mut() { *b = rng.gen(); }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

fn decrypt_pokedex(data: &[u8]) -> Option<Pokedex> {
    if data.len() < 13 || data[0] == b'{' { return None; }
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let nonce = Nonce::from_slice(&data[..12]);
    let plaintext = cipher.decrypt(nonce, &data[12..]).ok()?;
    let json_str = String::from_utf8(plaintext).ok()?;
    serde_json::from_str(&json_str).ok()
}

fn get_team_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("catch-pokemon");
    path.push("battle_team.json");
    path
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BattleTeamEntry {
    name: String,
    #[serde(default)]
    shiny: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct BattleTeam {
    pokemon: Vec<BattleTeamEntry>,
}

impl BattleTeam {
    fn new() -> Self {
        BattleTeam { pokemon: Vec::new() }
    }

    fn load() -> Self {
        let path = get_team_path();
        if !path.exists() {
            return BattleTeam::new();
        }

        if let Ok(encrypted_bytes) = fs::read(&path) {
            if let Some(team) = decrypt_battle_team(&encrypted_bytes) {
                return team;
            }
        }

        // Don't wipe — same protection as PC
        if path.exists() {
            let backup = path.with_extension("json.bak");
            if !backup.exists() {
                let _ = fs::copy(&path, &backup);
            }
            eprintln!("{}", "Could not decrypt battle team.".red().bold());
            eprintln!("{}", "Your team data has been backed up.".yellow());
            std::process::exit(1);
        }

        BattleTeam::new()
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_team_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encrypted = encrypt_battle_team(self)?;
        fs::write(&path, encrypted)?;
        Ok(())
    }
}

fn encrypt_battle_team(team: &BattleTeam) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let json = serde_json::to_string(team)?;
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    for b in nonce_bytes.iter_mut() { *b = rng.gen(); }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

fn decrypt_battle_team(data: &[u8]) -> Option<BattleTeam> {
    if data.len() < 13 || data[0] == b'{' {
        return None;
    }
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let nonce = Nonce::from_slice(&data[..12]);
    let plaintext = cipher.decrypt(nonce, &data[12..]).ok()?;
    let json_str = String::from_utf8(plaintext).ok()?;
    serde_json::from_str(&json_str).ok()
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


fn catch_pokemon(pokemon: String, skip_animation: bool, hide_pokemon: bool, shiny: bool, token: Option<String>, attempt: u32) {
    // Validate session token — prevents manual catching
    match &token {
        None => {
            println!("{}", "You can't catch Pokemon directly! Use 'pokemon_encounter' first, then 'catch'.".red().bold());
            return;
        }
        Some(t) => {
            // Token format: "timestamp:hmac_hex"
            let parts: Vec<&str> = t.splitn(2, ':').collect();
            if parts.len() != 2 {
                println!("{}", "Invalid session token.".red());
                return;
            }
            let timestamp: i64 = match parts[0].parse() {
                Ok(ts) => ts,
                Err(_) => {
                    println!("{}", "Invalid session token.".red());
                    return;
                }
            };

            // Check token is not too old (30 minutes max)
            let now = Local::now().timestamp();
            if (now - timestamp).abs() > 1800 {
                println!("{}", "Session expired. Start a new encounter with 'pokemon_encounter'.".red());
                return;
            }

            // Verify HMAC
            let token_data = format!("encounter:{}:{}", pokemon.to_lowercase(), timestamp);
            let key = derive_signing_key();
            let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&key)
                .expect("HMAC accepts any key length");
            mac.update(token_data.as_bytes());
            let expected = hex::encode(mac.finalize().into_bytes());

            if parts[1] != expected {
                println!("{}", "Invalid session token. Nice try.".red().bold());
                return;
            }
        }
    }

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

            // Track in Pokedex as caught
            let mut pokedex = Pokedex::load();
            pokedex.mark_caught(&pokemon);
            let _ = pokedex.save();
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

fn color_type(t: &str) -> String {
    match t {
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
}

fn color_category(cat: &str) -> String {
    match cat {
        "legendary"        => "Legendary".red().bold().to_string(),
        "mythical"         => "Mythical".magenta().bold().to_string(),
        "pseudo_legendary" => "Pseudo-Legendary".yellow().bold().to_string(),
        "starter"          => "Starter".green().bold().to_string(),
        "starter_evolution" => "Starter Evo".green().to_string(),
        "rare"             => "Rare".cyan().bold().to_string(),
        "baby"             => "Baby".bright_magenta().to_string(),
        "uncommon"         => "Uncommon".white().to_string(),
        "common"           => "Common".bright_black().to_string(),
        _                  => cat.to_string(),
    }
}

// Info about a unique Pokemon in the PC
struct PcEntry {
    name: String,
    count: usize,
    shiny_count: usize,
    types: Vec<String>,
    power_rank: u8,
    category: String,
    first_caught: String,
    last_caught: String,
    on_team: bool,
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

    if search {
        if let Err(e) = interactive_pokemon_search(&storage) {
            eprintln!("Error in interactive search: {}", e);
        }
        return;
    }

    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();

    // Load battle team to show which Pokemon are on it
    let battle_team = BattleTeam::load();
    let team_names: Vec<String> = battle_team.pokemon.iter()
        .map(|p| p.name.to_lowercase().replace("-", "_"))
        .collect();

    // Build entries grouped by name
    let mut entries_map: HashMap<String, PcEntry> = HashMap::new();
    for p in &storage.pokemon {
        let normalized = p.name.to_lowercase().replace("-", "_");
        let entry = entries_map.entry(p.name.clone()).or_insert_with(|| {
            let (types, power, category) = if let Some(data) = pokemon_db.get(&normalized) {
                (data.types.clone(), data.power_rank, data.category.clone())
            } else {
                (vec![], 0, "unknown".to_string())
            };
            PcEntry {
                name: p.name.clone(),
                count: 0,
                shiny_count: 0,
                types,
                power_rank: power,
                category,
                first_caught: p.caught_at.format("%Y-%m-%d %H:%M").to_string(),
                last_caught: p.caught_at.format("%Y-%m-%d %H:%M").to_string(),
                on_team: team_names.contains(&normalized),
            }
        });
        entry.count += 1;
        if p.shiny { entry.shiny_count += 1; }
        let ts = p.caught_at.format("%Y-%m-%d %H:%M").to_string();
        if ts < entry.first_caught { entry.first_caught = ts.clone(); }
        if ts > entry.last_caught { entry.last_caught = ts; }
    }

    let mut entries: Vec<PcEntry> = entries_map.into_values().collect();
    // Sort: shinies first, then alphabetical
    entries.sort_by(|a, b| {
        b.shiny_count.cmp(&a.shiny_count)
            .then(a.name.cmp(&b.name))
    });

    if entries.is_empty() {
        println!("{}", "Your PC is empty. Go catch some Pokemon!".yellow());
        return;
    }

    // Launch TUI
    if let Err(e) = pc_tui(&mut entries, &storage) {
        eprintln!("TUI error: {}", e);
    }
}

fn pc_tui(entries: &mut Vec<PcEntry>, _storage: &PcStorage) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};

    // Enter alternate screen (like vim does — clean slate, restores on exit)
    stdout().execute(EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    stdout().execute(cursor::Hide)?;

    let mut selected: usize = 0;
    let mut scroll_offset: usize = 0;
    // Cache sprite for current selection to avoid re-running command on every frame
    let mut cached_sprite_name = String::new();
    let mut cached_sprite: Vec<String> = Vec::new();
    let mut status_msg: Option<String> = None;
    let mut confirming_release = false;

    loop {
        let (tw, th) = terminal::size().unwrap_or((80, 24));
        let tw = tw as usize;
        let th = th as usize;
        let left_width = 28.min(tw / 3);
        let list_height = th.saturating_sub(4);

        // Get selected entry
        let sel = &entries[selected];

        // Load sprite only when selection changes
        // Show shiny sprite if the user has a shiny version
        let sprite_key = format!("{}:{}", sel.name, sel.shiny_count > 0);
        if cached_sprite_name != sprite_key {
            cached_sprite_name = sprite_key;
            let display_name = sel.name.replace("_", "-");
            let mut args = vec!["-n", &display_name, "--no-title"];
            if sel.shiny_count > 0 {
                args.push("-s");
            }
            cached_sprite = Command::new("pokemon-colorscripts")
                .args(&args)
                .output()
                .ok()
                .filter(|r| r.status.success())
                .map(|r| String::from_utf8_lossy(&r.stdout).lines().map(|l| l.to_string()).collect())
                .unwrap_or_else(|| vec!["(no sprite)".to_string()]);
        }

        // Build right panel content
        let types_display: Vec<String> = sel.types.iter().map(|t| color_type(t)).collect();
        let cat_display = color_category(&sel.category);

        let mut right: Vec<String> = Vec::new();
        right.push(format!("{}", sel.name.green().bold()));
        right.push(String::new());
        right.push(format!("Type:     {}", types_display.join(" / ")));
        right.push(format!("Power:    {}", format!("{}", sel.power_rank).bright_yellow().bold()));
        right.push(format!("Category: {}", cat_display));

        // Look up rates
        let normalized = sel.name.replace("-", "_");
        let pokemon_db_local: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();
        if let Some(data) = pokemon_db_local.get(&normalized) {
            let valid_names_set: std::collections::HashSet<&str> = VALID_POKEMON
                .lines().filter(|l| !l.is_empty()).collect();
            let total_weight: u32 = pokemon_db_local.iter()
                .filter(|(n, _)| valid_names_set.contains(n.as_str()))
                .map(|(_, d)| d.catch_rate as u32).sum();

            // Category encounter rate (all Pokemon in this category combined)
            let category_weight: u32 = pokemon_db_local.iter()
                .filter(|(n, d)| valid_names_set.contains(n.as_str()) && d.category == data.category)
                .map(|(_, d)| d.catch_rate as u32).sum();
            let category_encounter_pct = category_weight as f32 / total_weight as f32 * 100.0;

            // Individual encounter rate
            let encounter_pct = data.catch_rate as f32 / total_weight as f32 * 100.0;

            // Catch rate with Poke Ball
            let catch_pct = data.catch_rate as f32 / 255.0 * 100.0;

            // True catch rate uses category encounter rate
            let true_catch_pct = category_encounter_pct * catch_pct / 100.0;

            let catch_color = if catch_pct >= 75.0 { format!("{:.1}%", catch_pct).green() }
                else if catch_pct >= 30.0 { format!("{:.1}%", catch_pct).yellow() }
                else { format!("{:.1}%", catch_pct).red() };

            let fmt_small = |pct: f32| -> String {
                if pct >= 10.0 { format!("{:.1}%", pct) }
                else if pct >= 1.0 { format!("{:.2}%", pct) }
                else if pct >= 0.1 { format!("{:.3}%", pct) }
                else if pct >= 0.01 { format!("{:.4}%", pct) }
                else if pct >= 0.001 { format!("{:.5}%", pct) }
                else { format!("{:.6}%", pct) }
            };

            let enc_color = if category_encounter_pct >= 10.0 { fmt_small(category_encounter_pct).green() }
                else if category_encounter_pct >= 1.0 { fmt_small(category_encounter_pct).yellow() }
                else { fmt_small(category_encounter_pct).red() };

            let true_color = if true_catch_pct >= 10.0 { fmt_small(true_catch_pct).green() }
                else if true_catch_pct >= 1.0 { fmt_small(true_catch_pct).yellow() }
                else { fmt_small(true_catch_pct).red() };

            right.push(format!("Encounter:{} ({})", enc_color.bold(), fmt_small(encounter_pct).dimmed()));
            right.push(format!("Catch:    {}", catch_color.bold()));
            right.push(format!("True odds:{}", true_color.bold()));
            right.push(format!("Flee:     {}", format!("{}%", data.flee_rate).red()));
        }

        if sel.on_team {
            right.push(format!("{}", "[On Battle Team]".cyan().bold()));
        }
        right.push(String::new());
        right.push(format!("{}", format!("First: {}", sel.first_caught).dimmed()));
        right.push(format!("{}", format!("Last:  {}", sel.last_caught).dimmed()));
        right.push(String::new());

        // Poke Ball grid: shinies as gold stars, regulars as red balls, 4 per row
        let regular = sel.count.saturating_sub(sel.shiny_count);
        let balls_per_row = 6;
        let mut ball_icons: Vec<String> = Vec::new();
        // Shinies first
        for _ in 0..sel.shiny_count {
            ball_icons.push("\x1B[1;33m★\x1B[0m".to_string()); // gold star
        }
        // Then regulars
        for _ in 0..regular {
            ball_icons.push("\x1B[31m◓\x1B[0m".to_string()); // red ball
        }

        // Render in rows
        right.push(format!("Caught: {}", format!("{}", sel.count).yellow()));
        for row in ball_icons.chunks(balls_per_row) {
            right.push(format!("  {}", row.join(" ")));
        }
        right.push(String::new());

        // Tile sprites in a grid if terminal is wide enough
        if !cached_sprite.is_empty() && sel.count > 0 {
            // Calculate sprite width (longest line, ignoring ANSI codes)
            let strip_ansi = |s: &str| -> usize {
                let mut len = 0;
                let mut in_escape = false;
                for c in s.chars() {
                    if c == '\x1B' { in_escape = true; }
                    else if in_escape {
                        if c.is_alphabetic() { in_escape = false; }
                    } else {
                        len += 1;
                    }
                }
                len
            };
            let sprite_width = cached_sprite.iter().map(|l| strip_ansi(l)).max().unwrap_or(20);
            let sprite_height = cached_sprite.len();
            let right_panel_width = tw.saturating_sub(left_width + 3);

            // How many sprites fit across?
            let gap = 2; // space between sprites
            let sprites_per_row = ((right_panel_width + gap) / (sprite_width + gap)).max(1);

            // Cap at count, max 16 to keep it reasonable
            let total_sprites = sel.count.min(16);
            let num_rows = (total_sprites + sprites_per_row - 1) / sprites_per_row;

            // Only show grid if we have room (terminal tall/wide enough)
            if right_panel_width >= sprite_width && sprites_per_row >= 1 {
                for grid_row in 0..num_rows {
                    let sprites_this_row = (total_sprites - grid_row * sprites_per_row).min(sprites_per_row);

                    // For each line of the sprite height
                    for line_idx in 0..sprite_height {
                        let mut combined = String::new();
                        for s in 0..sprites_this_row {
                            if s > 0 {
                                combined.push_str(&" ".repeat(gap));
                            }
                            if line_idx < cached_sprite.len() {
                                combined.push_str(&cached_sprite[line_idx]);
                                // Pad to sprite_width (using visible width)
                                let visible = strip_ansi(&cached_sprite[line_idx]);
                                if visible < sprite_width {
                                    combined.push_str(&" ".repeat(sprite_width - visible));
                                }
                            }
                        }
                        right.push(combined);
                    }
                    if grid_row < num_rows - 1 {
                        right.push(String::new()); // gap between grid rows
                    }
                }
            } else {
                // Terminal too narrow, just show single sprite
                for line in &cached_sprite {
                    right.push(line.clone());
                }
            }
        }

        // Adjust scroll
        if selected >= scroll_offset + list_height {
            scroll_offset = selected + 1 - list_height;
        }
        if selected < scroll_offset {
            scroll_offset = selected;
        }

        // Render — move to top-left, write each line, clear to end of line
        stdout().execute(cursor::MoveTo(0, 0))?;

        // Header
        let header = format!(" {} ({} unique | {} caught)",
            "Pokemon PC".cyan().bold(),
            entries.len().to_string().yellow(),
            entries.iter().map(|e| e.count).sum::<usize>().to_string().yellow());
        print!("{}\x1B[K\r\n", header);
        print!(" {}\x1B[K\r\n", "─".repeat(tw.saturating_sub(2)).dimmed());

        // Body rows
        let name_width = left_width.saturating_sub(8); // space for " >*~ name xN"
        for row in 0..list_height {
            // Left panel
            let left = if row < entries.len().saturating_sub(scroll_offset).min(list_height) {
                let idx = scroll_offset + row;
                if idx < entries.len() {
                    let e = &entries[idx];
                    let arrow = if idx == selected { ">" } else { " " };
                    let team_mark = if e.on_team { "*" } else { " " };
                    let shiny_mark = if e.shiny_count > 0 { "~" } else { " " };
                    let count = if e.count > 1 { format!(" x{}", e.count) } else { String::new() };

                    // Truncate and pad name
                    let name_with_count = format!("{}{}", e.name, count);
                    let truncated: String = name_with_count.chars().take(name_width).collect();
                    let padded = format!("{:<width$}", truncated, width = name_width);

                    if idx == selected {
                        format!(" \x1B[7m{}{}{} {}\x1B[0m", arrow, team_mark, shiny_mark, padded)
                    } else {
                        format!(" {}{}{} \x1B[32m{}\x1B[0m", arrow, team_mark, shiny_mark, padded)
                    }
                } else {
                    format!("{:<width$}", "", width = left_width)
                }
            } else {
                format!("{:<width$}", "", width = left_width)
            };

            // Right panel
            let right_text = if row < right.len() {
                &right[row]
            } else {
                ""
            };

            // Write: left panel + separator + right panel + clear rest of line
            print!("{}\x1B[{}G\x1B[90m│\x1B[0m {}\x1B[K\r\n", left, left_width + 1, right_text);
        }

        // Footer
        print!(" {}\x1B[K\r\n", "─".repeat(tw.saturating_sub(2)).dimmed());
        if let Some(ref msg) = status_msg {
            print!(" {}\x1B[K", msg.red().bold());
            status_msg = None;
        } else {
            let team_count = entries.iter().filter(|e| e.on_team).count();
            print!(" {}\x1B[K",
                format!("↑↓ Navigate | T: Team ({}/20) | R: Release | Q: Quit", team_count).dimmed());
        }
        stdout().flush()?;

        // Drain queued events to prevent scroll/input lag
        while event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
            let _ = event::read();
        }

        // Input
        if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
            match code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Up | KeyCode::Char('k') => {
                    if selected > 0 { selected -= 1; }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected < entries.len() - 1 { selected += 1; }
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    let name = entries[selected].name.clone();
                    let normalized = name.to_lowercase().replace("-", "_");
                    let mut team = BattleTeam::load();
                    if team.pokemon.iter().any(|p| p.name.to_lowercase().replace("-", "_") == normalized) {
                        // Remove from team
                        team.pokemon.retain(|p| p.name.to_lowercase().replace("-", "_") != normalized);
                        let _ = team.save();
                        entries[selected].on_team = false;
                    } else if team.pokemon.len() >= 20 {
                        // Team is full — show warning in footer on next render
                        status_msg = Some("Battle team is full! (20/20) Remove one first.".to_string());
                    } else {
                        // Add to team
                        let is_shiny = entries[selected].shiny_count > 0;
                        team.pokemon.push(BattleTeamEntry { name: name.to_lowercase(), shiny: is_shiny });
                        let _ = team.save();
                        entries[selected].on_team = true;
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if entries.is_empty() { continue; }
                    let name = entries[selected].name.clone();
                    let count = entries[selected].count;

                    // Show confirmation in footer
                    status_msg = Some(format!(
                        "Release {}{}? Press Y to confirm, any other key to cancel",
                        name,
                        if count > 1 { " (releases 1)" } else { "" }
                    ));

                    // Render the confirmation message immediately
                    // (the loop will redraw, then we wait for the next keypress)
                    confirming_release = true;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') if confirming_release => {
                    let name = entries[selected].name.clone();
                    confirming_release = false;

                    // Release from PC storage
                    let mut storage = PcStorage::load();
                    let released = storage.release_pokemon(&name, 1);
                    if released > 0 {
                        if let Err(e) = storage.save() {
                            status_msg = Some(format!("Error saving: {}", e));
                        } else {
                            // Update entries in-place
                            entries[selected].count -= 1;
                            if entries[selected].count == 0 {
                                entries.remove(selected);
                                if selected > 0 && selected >= entries.len() {
                                    selected = entries.len() - 1;
                                }
                            }
                            // Also remove from battle team if count is 0
                            if entries.is_empty() || (selected < entries.len() && entries[selected].count == 0) {
                                let normalized = name.to_lowercase().replace("-", "_");
                                let mut team = BattleTeam::load();
                                team.pokemon.retain(|p| p.name.to_lowercase().replace("-", "_") != normalized);
                                let _ = team.save();
                            }
                            status_msg = Some(format!("{} released back to the wild!", name));
                            // Clear sprite cache so it reloads for new selection
                            cached_sprite_name = String::new();
                        }
                    }
                }
                _ if confirming_release => {
                    confirming_release = false;
                    status_msg = Some("Release cancelled.".to_string());
                }
                KeyCode::Home => { selected = 0; }
                KeyCode::End => {
                    if !entries.is_empty() { selected = entries.len() - 1; }
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    stdout().execute(cursor::Show)?;
    terminal::disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
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
    // 1/4096 chance of shiny encounter (0.024%)
    let is_shiny = rng.gen_range(0u32..4096) == 0;

    // Always print the name (for scripting use)
    println!("{}", display_name);

    // Track in Pokedex as seen
    let mut pokedex = Pokedex::load();
    pokedex.mark_seen(&display_name);
    let _ = pokedex.save();

    // Generate session token: HMAC(signing_key, pokemon_name + timestamp)
    // This proves the encounter was real — can't forge without the key
    let timestamp = Local::now().timestamp();
    let token_data = format!("encounter:{}:{}", display_name, timestamp);
    let key = derive_signing_key();
    let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&key)
        .expect("HMAC accepts any key length");
    mac.update(token_data.as_bytes());
    let token = format!("{}:{}", timestamp, hex::encode(mac.finalize().into_bytes()));

    // Print shiny status and token (for shell function)
    println!("Shiny: {}", is_shiny);
    println!("Token: {}", token);

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

fn restore_pc(file: Option<String>) {
    let backup_path = match file {
        Some(f) => PathBuf::from(f),
        None => get_storage_path().with_file_name("pc_backup.json"),
    };

    if !backup_path.exists() {
        eprintln!("{}", format!("Backup file not found: {}", backup_path.display()).red());
        eprintln!("If someone sent you a backup, use: catch-pokemon restore --file /path/to/backup.json");
        return;
    }

    let contents = match fs::read_to_string(&backup_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", format!("Could not read backup: {}", e).red());
            return;
        }
    };

    let mut storage: PcStorage = match serde_json::from_str(&contents) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", format!("Invalid backup JSON: {}", e).red());
            return;
        }
    };

    println!("{}", format!("Found {} Pokemon in backup.", storage.pokemon.len()).cyan());
    println!("{}", "This will re-sign and encrypt the data with the current key.".yellow());
    print!("Restore? (y/n) > ");
    stdout().flush().unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() != "y" {
        println!("Restore cancelled.");
        return;
    }

    // Re-sign the chain with the current key
    storage.resign_chain();

    if let Err(e) = storage.save() {
        eprintln!("{}", format!("Error saving restored PC: {}", e).red());
    } else {
        println!("{}", format!("Restored {} Pokemon! PC is encrypted and verified.", storage.pokemon.len()).green().bold());
    }
}

fn show_pokedex() {
    use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};

    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();
    let valid_names: std::collections::HashSet<&str> = VALID_POKEMON
        .lines()
        .filter(|l| !l.is_empty())
        .collect();

    let pokedex = Pokedex::load();

    // Build list of all valid Pokemon
    struct DexRow {
        name: String,
        types: Vec<String>,
        power_rank: u8,
        category: String,
        seen: bool,
        caught: bool,
        has_shiny: bool,
        seen_at: Option<String>,
        caught_at: Option<String>,
    }

    // Load PC to check for shinies
    let pc_storage = PcStorage::load();
    let shiny_pokemon: std::collections::HashSet<String> = pc_storage.pokemon.iter()
        .filter(|p| p.shiny)
        .map(|p| p.name.to_lowercase())
        .collect();

    let mut rows: Vec<DexRow> = Vec::new();
    for name in &valid_names {
        if let Some(data) = pokemon_db.get(*name) {
            let display_name = name.replace("_", "-");
            let entry = pokedex.entries.get(&display_name);
            rows.push(DexRow {
                name: display_name.clone(),
                types: data.types.clone(),
                power_rank: data.power_rank,
                category: data.category.clone(),
                seen: entry.map(|e| e.seen).unwrap_or(false),
                caught: entry.map(|e| e.caught).unwrap_or(false),
                has_shiny: shiny_pokemon.contains(&display_name),
                seen_at: entry.and_then(|e| e.seen_at.map(|t| t.format("%Y-%m-%d").to_string())),
                caught_at: entry.and_then(|e| e.caught_at.map(|t| t.format("%Y-%m-%d").to_string())),
            });
        }
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));

    let total = rows.len();
    let seen_count = rows.iter().filter(|r| r.seen).count();
    let caught_count = rows.iter().filter(|r| r.caught).count();

    // TUI
    stdout().execute(EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode().unwrap();
    stdout().execute(cursor::Hide).unwrap();

    let mut selected: usize = 0;
    let mut scroll_offset: usize = 0;
    let mut search_term = String::new();
    let mut searching = false;
    let mut cached_sprite_name = String::new();
    let mut cached_sprite: Vec<String> = Vec::new();

    loop {
        // Filter by search
        let filtered: Vec<&DexRow> = if search_term.is_empty() {
            rows.iter().collect()
        } else {
            let lower = search_term.to_lowercase();
            rows.iter().filter(|r| {
                r.name.contains(&lower)
            }).collect()
        };

        if selected >= filtered.len() && !filtered.is_empty() {
            selected = filtered.len() - 1;
        }

        let (tw, th) = terminal::size().unwrap_or((80, 24));
        let tw = tw as usize;
        let th = th as usize;
        let left_width = 30.min(tw / 3);
        let list_height = th.saturating_sub(6); // header + search + footer

        // Scroll
        if selected >= scroll_offset + list_height {
            scroll_offset = selected + 1 - list_height;
        }
        if selected < scroll_offset {
            scroll_offset = selected;
        }

        // Get selected row
        let sel = if !filtered.is_empty() { Some(filtered[selected]) } else { None };

        // Cache sprite (show shiny sprite if user has a shiny)
        if let Some(s) = sel {
            let sprite_key = format!("{}:{}", s.name, s.has_shiny);
            if cached_sprite_name != sprite_key {
                cached_sprite_name = sprite_key;
                let mut args = vec!["-n", &s.name, "--no-title"];
                if s.has_shiny {
                    args.push("-s");
                }
                cached_sprite = Command::new("pokemon-colorscripts")
                    .args(&args)
                    .output()
                    .ok()
                    .filter(|r| r.status.success())
                    .map(|r| String::from_utf8_lossy(&r.stdout).lines().map(|l| l.to_string()).collect())
                    .unwrap_or_default();
            }
        }

        // Build right panel
        let mut right: Vec<String> = Vec::new();
        if let Some(s) = sel {
            let types_display: Vec<String> = s.types.iter().map(|t| color_type(t)).collect();
            let cat_display = color_category(&s.category);

            right.push(format!("{}", s.name.green().bold()));
            right.push(String::new());
            right.push(format!("Type:     {}", types_display.join(" / ")));
            right.push(format!("Power:    {}", format!("{}", s.power_rank).bright_yellow().bold()));
            right.push(format!("Category: {}", cat_display));

            // Look up rates
            let normalized_dex = s.name.replace("-", "_");
            if let Some(data) = pokemon_db.get(&normalized_dex) {
                let total_weight: u32 = pokemon_db.iter()
                    .filter(|(n, _)| valid_names.contains(n.as_str()))
                    .map(|(_, d)| d.catch_rate as u32).sum();

                // Category encounter rate
                let category_weight: u32 = pokemon_db.iter()
                    .filter(|(n, d)| valid_names.contains(n.as_str()) && d.category == data.category)
                    .map(|(_, d)| d.catch_rate as u32).sum();
                let category_encounter_pct = category_weight as f32 / total_weight as f32 * 100.0;

                let encounter_pct = data.catch_rate as f32 / total_weight as f32 * 100.0;
                let catch_pct = data.catch_rate as f32 / 255.0 * 100.0;
                let true_catch_pct = category_encounter_pct * catch_pct / 100.0;

                let catch_color = if catch_pct >= 75.0 { format!("{:.1}%", catch_pct).green() }
                    else if catch_pct >= 30.0 { format!("{:.1}%", catch_pct).yellow() }
                    else { format!("{:.1}%", catch_pct).red() };

                let fmt_small = |pct: f32| -> String {
                    if pct >= 10.0 { format!("{:.1}%", pct) }
                    else if pct >= 1.0 { format!("{:.2}%", pct) }
                    else if pct >= 0.1 { format!("{:.3}%", pct) }
                    else if pct >= 0.01 { format!("{:.4}%", pct) }
                    else if pct >= 0.001 { format!("{:.5}%", pct) }
                    else { format!("{:.6}%", pct) }
                };

                let enc_color = if category_encounter_pct >= 10.0 { fmt_small(category_encounter_pct).green() }
                    else if category_encounter_pct >= 1.0 { fmt_small(category_encounter_pct).yellow() }
                    else { fmt_small(category_encounter_pct).red() };

                let true_color = if true_catch_pct >= 10.0 { fmt_small(true_catch_pct).green() }
                    else if true_catch_pct >= 1.0 { fmt_small(true_catch_pct).yellow() }
                    else { fmt_small(true_catch_pct).red() };

                right.push(format!("Encounter:{} ({})", enc_color.bold(), fmt_small(encounter_pct).dimmed()));
                right.push(format!("Catch:    {}", catch_color.bold()));
                right.push(format!("True odds:{}", true_color.bold()));
                right.push(format!("Flee:     {}", format!("{}%", data.flee_rate).red()));
            }

            right.push(String::new());

            if s.caught {
                if s.has_shiny {
                    right.push(format!("{} {}", "Caught".green().bold(), "[Shiny]".yellow().bold()));
                } else {
                    right.push(format!("{}", "Caught".green().bold()));
                }
                if let Some(ref d) = s.caught_at {
                    right.push(format!("{}", format!("Caught: {}", d).dimmed()));
                }
            } else if s.seen {
                right.push(format!("{}", "Seen (not caught)".yellow()));
                if let Some(ref d) = s.seen_at {
                    right.push(format!("{}", format!("Seen: {}", d).dimmed()));
                }
            } else {
                right.push(format!("{}", "Not discovered".dimmed()));
            }

            right.push(String::new());
            for line in &cached_sprite {
                right.push(line.clone());
            }
        }

        // Render
        stdout().execute(cursor::MoveTo(0, 0)).unwrap();

        // Header
        print!(" {} | {}/{} seen | {}/{} caught\x1B[K\r\n",
            "Pokedex".cyan().bold(),
            format!("{}", seen_count).yellow(),
            total,
            format!("{}", caught_count).green(),
            total);

        // Search bar
        if searching {
            print!(" {}: {}{}\x1B[K\r\n",
                "Search".cyan().bold(),
                search_term.yellow(),
                "▌".yellow());
        } else if !search_term.is_empty() {
            print!(" Search: {}\x1B[K\r\n", search_term.yellow());
        } else {
            print!(" {}\x1B[K\r\n", "Press / to search".dimmed());
        }
        print!(" {}\x1B[K\r\n", "─".repeat(tw.saturating_sub(2)).dimmed());

        // Body
        let name_width = left_width.saturating_sub(6); // space for " X X name"
        for row in 0..list_height {
            let left = if row < filtered.len().saturating_sub(scroll_offset).min(list_height) {
                let idx = scroll_offset + row;
                if idx < filtered.len() {
                    let r = filtered[idx];
                    let status = if r.caught && r.has_shiny {
                        "\x1B[33m★\x1B[0m"  // gold star = caught + shiny
                    } else if r.caught {
                        "\x1B[32m●\x1B[0m"  // green dot = caught
                    } else if r.seen {
                        "\x1B[33m◐\x1B[0m"  // yellow half = seen
                    } else {
                        "\x1B[90m○\x1B[0m"  // gray empty = unknown
                    };

                    // Truncate name to fit
                    let truncated: String = r.name.chars().take(name_width).collect();
                    let padded = format!("{:<width$}", truncated, width = name_width);

                    if idx == selected {
                        format!(" \x1B[7m {} {}\x1B[0m", status, padded)
                    } else {
                        format!("  {} \x1B[32m{}\x1B[0m", status, padded)
                    }
                } else {
                    format!("{:<width$}", "", width = left_width)
                }
            } else {
                format!("{:<width$}", "", width = left_width)
            };

            let right_text = if row < right.len() {
                &right[row]
            } else {
                ""
            };

            print!("{}\x1B[{}G\x1B[90m│\x1B[0m {}\x1B[K\r\n", left, left_width + 1, right_text);
        }

        // Footer
        print!(" {}\x1B[K\r\n", "─".repeat(tw.saturating_sub(2)).dimmed());
        if searching {
            print!(" {} | {}\x1B[K",
                format!("{}/{}", if filtered.is_empty() { 0 } else { selected + 1 }, filtered.len()).dimmed(),
                "Type to filter | ↑↓ Navigate | Esc: Stop searching".dimmed());
        } else {
            print!(" {} | {}\x1B[K",
                format!("{}/{}", if filtered.is_empty() { 0 } else { selected + 1 }, filtered.len()).dimmed(),
                "↑↓ Navigate | /: Search | Q: Quit".dimmed());
        }
        stdout().flush().unwrap();

        // Drain queued events to prevent scroll/input lag
        while event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
            let _ = event::read();
        }

        // Input
        if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
            if searching {
                // Search mode: typing filters, Esc exits search
                match code {
                    KeyCode::Esc => {
                        searching = false;
                    }
                    KeyCode::Enter => {
                        searching = false;
                    }
                    KeyCode::Backspace => {
                        search_term.pop();
                        selected = 0;
                        scroll_offset = 0;
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up => {
                        if selected > 0 { selected -= 1; }
                    }
                    KeyCode::Down => {
                        if !filtered.is_empty() && selected < filtered.len() - 1 { selected += 1; }
                    }
                    KeyCode::Char(c) => {
                        search_term.push(c);
                        selected = 0;
                        scroll_offset = 0;
                    }
                    _ => {}
                }
            } else {
                // Normal mode: navigate, / to search, q to quit
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('/') => {
                        searching = true;
                        search_term.clear();
                        selected = 0;
                        scroll_offset = 0;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if selected > 0 { selected -= 1; }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !filtered.is_empty() && selected < filtered.len() - 1 { selected += 1; }
                    }
                    KeyCode::Home => { selected = 0; }
                    KeyCode::End => {
                        if !filtered.is_empty() { selected = filtered.len() - 1; }
                    }
                    _ => {}
                }
            }
        }
    }

    stdout().execute(cursor::Show).unwrap();
    terminal::disable_raw_mode().unwrap();
    stdout().execute(LeaveAlternateScreen).unwrap();
}

fn manage_team(add: Option<String>, remove: Option<String>, clear: bool) {
    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();

    if clear {
        let team = BattleTeam::new();
        if let Err(e) = team.save() {
            eprintln!("{}", format!("Error clearing team: {}", e).red());
        } else {
            println!("{}", "Battle team cleared.".green());
        }
        return;
    }

    if let Some(name) = add {
        let pc = PcStorage::load();
        let normalized = name.to_lowercase().replace("-", "_");

        // Check if Pokemon exists in PC
        if !pc.pokemon.iter().any(|p| p.name.to_lowercase().replace("-", "_") == normalized) {
            println!("{}", format!("You don't have {} in your PC.", name).red());
            return;
        }

        let mut team = BattleTeam::load();

        if team.pokemon.len() >= 20 {
            println!("{}", "Battle team is full (20 Pokemon max). Remove one first.".red());
            return;
        }

        // Check if already on team
        if team.pokemon.iter().any(|p| p.name.to_lowercase().replace("-", "_") == normalized) {
            println!("{}", format!("{} is already on your battle team.", name).yellow());
            return;
        }

        // Find if any are shiny
        let is_shiny = pc.pokemon.iter()
            .any(|p| p.name.to_lowercase().replace("-", "_") == normalized && p.shiny);

        team.pokemon.push(BattleTeamEntry {
            name: name.to_lowercase(),
            shiny: is_shiny,
        });

        if let Err(e) = team.save() {
            eprintln!("{}", format!("Error saving team: {}", e).red());
        } else {
            println!("{}", format!("{} added to battle team. ({}/20)", name, team.pokemon.len()).green());
        }
        return;
    }

    if let Some(name) = remove {
        let mut team = BattleTeam::load();
        let normalized = name.to_lowercase().replace("-", "_");
        let before = team.pokemon.len();
        team.pokemon.retain(|p| p.name.to_lowercase().replace("-", "_") != normalized);

        if team.pokemon.len() == before {
            println!("{}", format!("{} is not on your battle team.", name).yellow());
        } else {
            if let Err(e) = team.save() {
                eprintln!("{}", format!("Error saving team: {}", e).red());
            } else {
                println!("{}", format!("{} removed from battle team. ({}/20)", name, team.pokemon.len()).green());
            }
        }
        return;
    }

    // Display current team
    let team = BattleTeam::load();

    if team.pokemon.is_empty() {
        println!("{}", "Your battle team is empty.".yellow());
        println!("Add Pokemon with: catch-pokemon team --add <name>");
        return;
    }

    println!();
    println!("{}", "  Battle Team".cyan().bold());
    println!("{}", "  ═══════════".cyan());
    println!();

    let mut total_power = 0u32;

    for (i, entry) in team.pokemon.iter().enumerate() {
        let normalized = entry.name.replace("-", "_");
        let (types_str, power, cat_display) = if let Some(data) = pokemon_db.get(&normalized) {
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
            let cat = match data.category.as_str() {
                "legendary"        => "Legendary".red().bold().to_string(),
                "mythical"         => "Mythical".magenta().bold().to_string(),
                "pseudo_legendary" => "Pseudo-Legendary".yellow().bold().to_string(),
                "starter"          => "Starter".green().bold().to_string(),
                "starter_evolution" => "Starter Evo".green().to_string(),
                "rare"             => "Rare".cyan().bold().to_string(),
                "baby"             => "Baby".bright_magenta().to_string(),
                "uncommon"         => "Uncommon".white().to_string(),
                "common"           => "Common".bright_black().to_string(),
                _                  => data.category.clone(),
            };
            (type_strings.join(" / "), data.power_rank as u32, cat)
        } else {
            ("???".to_string(), 0, "unknown".to_string())
        };

        total_power += power;

        let shiny_str = if entry.shiny { " [Shiny]".yellow().bold().to_string() } else { String::new() };

        println!("  [{}] {}{}", format!("{:2}", i + 1).dimmed(), entry.name.green().bold(), shiny_str);
        println!("      {} | Power: {} | {}", types_str, format!("{}", power).bright_yellow().bold(), cat_display);
    }

    println!();
    println!("  {}", "──────────────────────────────────────".dimmed());
    println!();
    println!("  {} | Total Power: {}",
        format!("{}/20 slots", team.pokemon.len()).cyan(),
        format!("{}", total_power).bright_yellow().bold(),
    );
}

fn update_binary() {
    println!("{}", "Checking for updates...".cyan());

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: {}", current_version.cyan());

    // Detect platform
    let os = if cfg!(target_os = "macos") { "macos" } else if cfg!(target_os = "linux") { "linux" } else { "unknown" };
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };
    let suffix = format!("{}-{}", os, arch);

    // Fetch latest release tag
    let output = Command::new("curl")
        .args(&["-sSL", "https://api.github.com/repos/matthewmyrick/catch-pokemon/releases/latest"])
        .output();

    let latest_tag = match output {
        Ok(o) => {
            let body = String::from_utf8_lossy(&o.stdout);
            body.lines()
                .find(|l| l.contains("tag_name"))
                .and_then(|l| l.split('"').nth(3))
                .map(|s| s.to_string())
        }
        Err(_) => None,
    };

    let tag = match latest_tag {
        Some(t) => t,
        None => {
            eprintln!("{}", "Could not fetch latest release.".red());
            return;
        }
    };

    let tag_version = tag.trim_start_matches('v');
    if tag_version == current_version {
        println!("{}", format!("Already on the latest version ({})", current_version).green());
        return;
    }

    println!("New version available: {}", tag.green().bold());

    // Download to temp file
    let archive = format!("catch-pokemon-{}-{}.tar.gz", tag, suffix);
    let url = format!("https://github.com/matthewmyrick/catch-pokemon/releases/download/{}/{}", tag, archive);

    println!("{}", format!("Downloading {}...", tag).yellow());

    let tmp_dir = std::env::temp_dir().join("catch-pokemon-update");
    let _ = fs::create_dir_all(&tmp_dir);
    let tmp_archive = tmp_dir.join(&archive);

    let dl_status = Command::new("curl")
        .args(&["-sSL", "--fail", "-o", tmp_archive.to_str().unwrap(), &url])
        .status();

    if !matches!(dl_status, Ok(s) if s.success()) {
        eprintln!("{}", format!("Download failed for {}", suffix).red());
        let _ = fs::remove_dir_all(&tmp_dir);
        return;
    }

    // Extract
    let tar_status = Command::new("tar")
        .args(&["-xzf", tmp_archive.to_str().unwrap(), "-C", tmp_dir.to_str().unwrap()])
        .status();

    if !matches!(tar_status, Ok(s) if s.success()) {
        eprintln!("{}", "Failed to extract update.".red());
        let _ = fs::remove_dir_all(&tmp_dir);
        return;
    }

    // Replace binary: remove old, move new
    let bin_dir = dirs::home_dir().unwrap().join(".local/bin");
    let bin_path = bin_dir.join("catch-pokemon");
    let new_binary = tmp_dir.join("catch-pokemon");

    let _ = fs::remove_file(&bin_path);
    if let Err(e) = fs::rename(&new_binary, &bin_path) {
        // rename may fail across filesystems, fall back to copy
        if let Err(e2) = fs::copy(&new_binary, &bin_path) {
            eprintln!("{}", format!("Failed to install update: {}", e2).red());
            let _ = fs::remove_dir_all(&tmp_dir);
            return;
        }
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755));
    }

    let _ = fs::remove_dir_all(&tmp_dir);

    println!("{}", format!("Updated to {} successfully!", tag).green().bold());

    // Run setup automatically with the new binary
    println!("{}", "Updating shell functions...".cyan());
    let _ = Command::new(bin_path.to_str().unwrap_or("catch-pokemon"))
        .arg("setup")
        .status();

    println!();
    println!("{}", "To start playing with the new version, run:".green());
    println!("  {}", "source ~/.zshrc && pokemon_new".cyan().bold());
}

fn main() {
    let args = Args::parse();
    
    match args.command {
        Commands::Catch { pokemon, skip_animation, hide_pokemon, shiny, token, attempt } => {
            catch_pokemon(pokemon, skip_animation, hide_pokemon, shiny, token, attempt);
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
        Commands::Pokedex => {
            show_pokedex();
        },
        Commands::Restore { file } => {
            restore_pc(file);
        },
        Commands::Team { add, remove, clear } => {
            manage_team(add, remove, clear);
        },
        Commands::Encounter { show_pokemon } => {
            encounter_pokemon(show_pokemon);
        },
        Commands::Update => {
            update_binary();
        }
    }
}