use colored::*;
use hmac::Mac as HmacMac;
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;

use crate::crypto::{derive_signing_key, verify_chain, HmacSha256};
use crate::models::PcStorage;

pub fn get_storage_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("catch-pokemon");
    path.push("pc_storage.json");
    path
}

pub fn get_pokedex_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("catch-pokemon");
    path.push("pokedex.json");
    path
}

pub fn get_team_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("catch-pokemon");
    path.push("battle_team.json");
    path
}

pub fn restore_pc(file: Option<String>) {
    let backup_path = match file {
        Some(f) => PathBuf::from(f),
        None => get_storage_path().with_file_name("pc_backup.json"),
    };

    if !backup_path.exists() {
        eprintln!(
            "{}",
            format!("Backup file not found: {}", backup_path.display()).red()
        );
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

    // Try signed backup format first
    let mut storage: PcStorage =
        if let Ok(signed) = serde_json::from_str::<serde_json::Value>(&contents) {
            if let (Some(data), Some(sig)) = (signed.get("data"), signed.get("signature")) {
                // Verify signature
                let data_json = serde_json::to_string(data).unwrap_or_default();
                let key = derive_signing_key();
                let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&key)
                    .expect("HMAC accepts any key length");
                mac.update(data_json.as_bytes());
                let expected_sig = hex::encode(mac.finalize().into_bytes());

                let actual_sig = sig.as_str().unwrap_or("");
                if actual_sig != expected_sig {
                    eprintln!(
                        "{}",
                        "Backup signature verification FAILED.".red().bold()
                    );
                    eprintln!("{}", "The backup file has been tampered with.".red());
                    return;
                }

                match serde_json::from_value(data.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{}", format!("Invalid backup data: {}", e).red());
                        return;
                    }
                }
            } else {
                // Unsigned legacy backup — allow for migration but warn
                match serde_json::from_str(&contents) {
                    Ok(s) => {
                        println!(
                            "{}",
                            "Warning: Unsigned backup (legacy format). Accepting for migration."
                                .yellow()
                        );
                        s
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Invalid backup JSON: {}", e).red());
                        return;
                    }
                }
            }
        } else {
            eprintln!("{}", "Could not parse backup file.".red());
            return;
        };

    println!(
        "{}",
        format!("Found {} Pokemon in backup.", storage.pokemon.len()).cyan()
    );
    println!(
        "{}",
        "This will re-sign and encrypt the data with the current key.".yellow()
    );
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
        println!(
            "{}",
            format!(
                "Restored {} Pokemon! PC is encrypted and verified.",
                storage.pokemon.len()
            )
            .green()
            .bold()
        );
        println!("{}", "Signed backup saved for recovery.".dimmed());
    }
}

pub fn clear_pc() {
    println!(
        "{}",
        "Are you sure you want to clear your PC? This cannot be undone!"
            .red()
            .bold()
    );
    print!("Type 'yes' to confirm: ");
    stdout().flush().unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() == "yes" {
        let path = get_storage_path();
        let dir = path.parent().unwrap().to_path_buf();

        // Remove all game data files
        let files = [
            "pc_storage.json",
            "pc_storage.json.bak",
            "pc_backup.json",
            "battle_team.json",
            "battle_team.json.bak",
            "pokedex.json",
            "pokedex.json.bak",
            "pokedex_backup.json",
        ];

        let mut cleared = false;
        for file in &files {
            let p = dir.join(file);
            if p.exists() {
                let _ = fs::remove_file(&p);
                cleared = true;
            }
        }

        if cleared {
            println!("{}", "All game data cleared!".green());
        } else {
            println!("PC was already empty.");
        }
    } else {
        println!("Clear cancelled.");
    }
}

pub fn verify_pc() {
    // Use PcStorage::load which handles decryption
    let storage = PcStorage::load();

    if storage.pokemon.is_empty() {
        println!("{}", "PC is empty. Nothing to verify.".yellow());
        return;
    }

    if storage.chain_hash.is_none() {
        println!(
            "{}",
            "Storage is unsigned (legacy format).".yellow()
        );
        return;
    }

    match verify_chain(&storage) {
        Ok(()) => {
            println!(
                "{}",
                format!(
                    "Integrity check PASSED. All {} entries verified.",
                    storage.pokemon.len()
                )
                .green()
                .bold()
            );
        }
        Err(msg) => {
            println!(
                "{}",
                format!("Integrity check FAILED: {}", msg).red().bold()
            );
            std::process::exit(1);
        }
    }
}
