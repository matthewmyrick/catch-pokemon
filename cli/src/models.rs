use chrono::{DateTime, Local};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::crypto::{
    compute_entry_hash, decrypt_battle_team, decrypt_pokedex, decrypt_storage,
    derive_signing_key, encrypt_battle_team, encrypt_pokedex, encrypt_storage,
    sign_entry, HmacSha256,
};
use crate::storage::{get_pokedex_path, get_storage_path, get_team_path};
use hmac::Mac as HmacMac;

// Embed the art files directly in the binary
pub const POKEBALL_STILL: &str = include_str!("../static/art/pokeball-still.txt");
pub const POKEBALL_LEFT: &str = include_str!("../static/art/pokeball-left.txt");
pub const POKEBALL_RIGHT: &str = include_str!("../static/art/pokeball-right.txt");
pub const POKEBALL_CAUGHT: &str = include_str!("../static/art/pokeball-caught.txt");
pub const POKEBALL_NOT_CAUGHT: &str = include_str!("../static/art/pokeball-not-caught.txt");

// Embed the Pokemon data directly in the binary
pub const POKEMON_DATA: &str = include_str!("../data/pokemon.json");

// Embed valid pokemon-colorscripts names (for encounter filtering)
pub const VALID_POKEMON: &str = include_str!("../data/valid_pokemon.txt");

// Embed the shell functions directly in the binary
pub const SHELL_FUNCTIONS: &str = include_str!("../shell/functions.sh");

pub fn default_flee_rate() -> u8 {
    10
}

#[derive(Debug, Clone, Copy)]
pub enum PokeballType {
    Pokeball,
}

impl PokeballType {
    pub fn catch_modifier(&self) -> f32 {
        match self {
            PokeballType::Pokeball => 1.0,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            PokeballType::Pokeball => "Poké Ball",
        }
    }

    pub fn ball_symbol(&self) -> String {
        match self {
            PokeballType::Pokeball => "◓".red().to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PokemonData {
    pub catch_rate: u8,
    pub category: String,
    #[serde(default = "default_flee_rate")]
    pub flee_rate: u8,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub power_rank: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CaughtPokemon {
    pub name: String,
    pub caught_at: DateTime<Local>,
    pub ball_used: String,
    #[serde(default)]
    pub shiny: bool,
    #[serde(default)]
    pub prev_hash: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PcStorage {
    pub pokemon: Vec<CaughtPokemon>,
    #[serde(default)]
    pub chain_hash: Option<String>,
}

impl PcStorage {
    pub fn new() -> Self {
        PcStorage {
            pokemon: Vec::new(),
            chain_hash: None,
        }
    }

    pub fn load() -> Self {
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

        // Try to read as legacy unencrypted JSON
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
        let backup_path = path.with_extension("json.bak");
        if !backup_path.exists() {
            let _ = fs::copy(&path, &backup_path);
            eprintln!(
                "{}",
                format!("PC storage backed up to {}", backup_path.display()).yellow()
            );
        }

        eprintln!("{}", "Could not decrypt PC storage.".red().bold());
        eprintln!(
            "{}",
            "If you have a backup, run: catch-pokemon restore".red()
        );
        eprintln!(
            "{}",
            "Or start fresh with: catch-pokemon clear".red()
        );
        eprintln!(
            "{}",
            "Your Pokemon data has been backed up and is NOT lost.".yellow()
        );
        eprintln!("To fix: rebuild with the correct BUILD_SECRET_KEY or restore the backup.");
        std::process::exit(1);
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_storage_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let encrypted = encrypt_storage(self)?;
        fs::write(&path, encrypted)?;

        // Write signed plaintext backup for recovery
        // The backup includes an HMAC signature so edits are detected on restore
        let backup_path = path.with_file_name("pc_backup.json");
        let data_json = serde_json::to_string(&self)?;
        let key = derive_signing_key();
        let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&key)
            .expect("HMAC accepts any key length");
        mac.update(data_json.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());

        let backup = serde_json::json!({
            "data": self,
            "signature": sig
        });
        fs::write(&backup_path, serde_json::to_string_pretty(&backup)?)?;

        Ok(())
    }

    pub fn add_pokemon(&mut self, name: String, ball: PokeballType, shiny: bool) {
        let key = derive_signing_key();
        let prev_hash = self
            .chain_hash
            .clone()
            .unwrap_or_else(|| "genesis".to_string());

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

    pub fn release_pokemon(&mut self, name: &str, count: usize) -> usize {
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

    pub fn resign_chain(&mut self) {
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

    pub fn has_pokemon(&self, name: &str) -> bool {
        self.pokemon
            .iter()
            .any(|p| p.name.to_lowercase() == name.to_lowercase())
    }

    pub fn count_pokemon(&self, name: &str) -> usize {
        self.pokemon
            .iter()
            .filter(|p| p.name.to_lowercase() == name.to_lowercase())
            .count()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PokedexEntry {
    pub name: String,
    pub seen: bool,
    pub caught: bool,
    pub seen_at: Option<DateTime<Local>>,
    pub caught_at: Option<DateTime<Local>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Pokedex {
    pub entries: HashMap<String, PokedexEntry>,
}

impl Pokedex {
    pub fn new() -> Self {
        Pokedex {
            entries: HashMap::new(),
        }
    }

    pub fn load() -> Self {
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

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_pokedex_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encrypted = encrypt_pokedex(self)?;
        fs::write(&path, encrypted)?;
        Ok(())
    }

    pub fn mark_seen(&mut self, name: &str) {
        let normalized = name.to_lowercase();
        let entry = self
            .entries
            .entry(normalized.clone())
            .or_insert_with(|| PokedexEntry {
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

    pub fn mark_caught(&mut self, name: &str) {
        let normalized = name.to_lowercase();
        let entry = self
            .entries
            .entry(normalized.clone())
            .or_insert_with(|| PokedexEntry {
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BattleTeamEntry {
    pub name: String,
    #[serde(default)]
    pub shiny: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BattleTeam {
    pub pokemon: Vec<BattleTeamEntry>,
}

impl BattleTeam {
    pub fn new() -> Self {
        BattleTeam {
            pokemon: Vec::new(),
        }
    }

    pub fn load() -> Self {
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

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_team_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encrypted = encrypt_battle_team(self)?;
        fs::write(&path, encrypted)?;
        Ok(())
    }
}

// Info about a unique Pokemon in the PC (used by pc_tui)
pub struct PcEntry {
    pub name: String,
    pub count: usize,
    pub shiny_count: usize,
    pub types: Vec<String>,
    pub power_rank: u8,
    pub category: String,
    pub first_caught: String,
    pub last_caught: String,
    pub on_team: bool,
}
