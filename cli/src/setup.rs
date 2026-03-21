use colored::*;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::models::{
    BattleTeam, BattleTeamEntry, PcStorage, PokemonData, POKEMON_DATA, SHELL_FUNCTIONS,
};

pub fn setup_shell() {
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
    println!(
        "{}",
        format!("Shell functions installed to {}", functions_path.display()).green()
    );

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
            println!(
                "{}",
                format!(
                    "Shell config already configured in {}",
                    config_path.display()
                )
                .green()
            );
            println!();
            println!(
                "{}",
                "Setup complete! Restart your terminal or run:".cyan().bold()
            );
            println!("  source {}", config_path.display());
            return;
        }
    }

    // Append source line
    let addition = format!(
        "\n# catch-pokemon shell functions (catch, pc, pokemon_encounter, etc.)\n{}\n",
        source_line
    );
    if let Err(e) = fs::OpenOptions::new()
        .append(true)
        .open(&config_path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(addition.as_bytes())
        })
    {
        eprintln!(
            "{}",
            format!("Error updating {}: {}", config_path.display(), e).red()
        );
        println!("Add this line manually:");
        println!("  {}", source_line);
        return;
    }

    println!(
        "{}",
        format!("Added to {}", config_path.display()).green()
    );
    println!();
    println!(
        "{}",
        "Setup complete! Restart your terminal or run:".cyan().bold()
    );
    println!("  source {}", config_path.display());
}

pub fn update_binary(pinned_version: Option<String>) {
    println!("{}", "Checking for updates...".cyan());

    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: {}", current_version.cyan());

    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    let suffix = format!("{}-{}", os, arch);

    let tag = if let Some(v) = pinned_version {
        // Ensure it starts with 'v'
        if v.starts_with('v') {
            v
        } else {
            format!("v{}", v)
        }
    } else {
        // Fetch latest release tag
        let output = Command::new("curl")
            .args(&[
                "-sSL",
                "https://api.github.com/repos/matthewmyrick/catch-pokemon/releases/latest",
            ])
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

        match latest_tag {
            Some(t) => t,
            None => {
                eprintln!("{}", "Could not fetch latest release.".red());
                return;
            }
        }
    };

    let tag_version = tag.trim_start_matches('v');
    if tag_version == current_version {
        println!(
            "{}",
            format!("Already on version {}", current_version).green()
        );
        return;
    }

    println!("Installing version: {}", tag.green().bold());

    // Download to temp file
    let archive = format!("catch-pokemon-{}-{}.tar.gz", tag, suffix);
    let url = format!(
        "https://github.com/matthewmyrick/catch-pokemon/releases/download/{}/{}",
        tag, archive
    );

    println!("{}", format!("Downloading {}...", tag).yellow());

    let tmp_dir = std::env::temp_dir().join("catch-pokemon-update");
    let _ = fs::create_dir_all(&tmp_dir);
    let tmp_archive = tmp_dir.join(&archive);

    let dl_status = Command::new("curl")
        .args(&[
            "-sSL",
            "--fail",
            "-o",
            tmp_archive.to_str().unwrap(),
            &url,
        ])
        .status();

    if !matches!(dl_status, Ok(s) if s.success()) {
        eprintln!("{}", format!("Download failed for {}", suffix).red());
        let _ = fs::remove_dir_all(&tmp_dir);
        return;
    }

    // Extract
    let tar_status = Command::new("tar")
        .args(&[
            "-xzf",
            tmp_archive.to_str().unwrap(),
            "-C",
            tmp_dir.to_str().unwrap(),
        ])
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
    if let Err(_e) = fs::rename(&new_binary, &bin_path) {
        // rename may fail across filesystems, fall back to copy
        if let Err(e2) = fs::copy(&new_binary, &bin_path) {
            eprintln!(
                "{}",
                format!("Failed to install update: {}", e2).red()
            );
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

    println!(
        "{}",
        format!("Updated to {} successfully!", tag).green().bold()
    );

    // Run setup automatically with the new binary
    println!("{}", "Updating shell functions...".cyan());
    let _ = Command::new(bin_path.to_str().unwrap_or("catch-pokemon"))
        .arg("setup")
        .status();

    println!();
    println!(
        "{}",
        "To start playing with the new version, run:".green()
    );
    println!("  {}", "source ~/.zshrc && pokemon_new".cyan().bold());
}

pub fn manage_team(add: Option<String>, remove: Option<String>, clear: bool) {
    let pokemon_db: HashMap<String, PokemonData> =
        serde_json::from_str(POKEMON_DATA).unwrap_or_default();

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
        if !pc
            .pokemon
            .iter()
            .any(|p| p.name.to_lowercase().replace("-", "_") == normalized)
        {
            println!(
                "{}",
                format!("You don't have {} in your PC.", name).red()
            );
            return;
        }

        let mut team = BattleTeam::load();

        if team.pokemon.len() >= 20 {
            println!(
                "{}",
                "Battle team is full (20 Pokemon max). Remove one first.".red()
            );
            return;
        }

        // Check if already on team
        if team
            .pokemon
            .iter()
            .any(|p| p.name.to_lowercase().replace("-", "_") == normalized)
        {
            println!(
                "{}",
                format!("{} is already on your battle team.", name).yellow()
            );
            return;
        }

        // Find if any are shiny
        let is_shiny = pc
            .pokemon
            .iter()
            .any(|p| p.name.to_lowercase().replace("-", "_") == normalized && p.shiny);

        team.pokemon.push(BattleTeamEntry {
            name: name.to_lowercase(),
            shiny: is_shiny,
        });

        if let Err(e) = team.save() {
            eprintln!("{}", format!("Error saving team: {}", e).red());
        } else {
            println!(
                "{}",
                format!("{} added to battle team. ({}/20)", name, team.pokemon.len()).green()
            );
        }
        return;
    }

    if let Some(name) = remove {
        let mut team = BattleTeam::load();
        let normalized = name.to_lowercase().replace("-", "_");
        let before = team.pokemon.len();
        team.pokemon
            .retain(|p| p.name.to_lowercase().replace("-", "_") != normalized);

        if team.pokemon.len() == before {
            println!(
                "{}",
                format!("{} is not on your battle team.", name).yellow()
            );
        } else {
            if let Err(e) = team.save() {
                eprintln!("{}", format!("Error saving team: {}", e).red());
            } else {
                println!(
                    "{}",
                    format!(
                        "{} removed from battle team. ({}/20)",
                        name,
                        team.pokemon.len()
                    )
                    .green()
                );
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
            let type_strings: Vec<String> = data
                .types
                .iter()
                .map(|t| match t.as_str() {
                    "fire" => t.red().bold().to_string(),
                    "water" => t.blue().bold().to_string(),
                    "grass" => t.green().bold().to_string(),
                    "electric" => t.yellow().bold().to_string(),
                    "ice" => t.cyan().bold().to_string(),
                    "fighting" => t.red().to_string(),
                    "poison" => t.purple().to_string(),
                    "ground" => t.yellow().to_string(),
                    "flying" => t.cyan().to_string(),
                    "psychic" => t.magenta().bold().to_string(),
                    "bug" => t.green().to_string(),
                    "rock" => t.yellow().dimmed().to_string(),
                    "ghost" => t.purple().bold().to_string(),
                    "dragon" => t.blue().bold().to_string(),
                    "dark" => t.white().dimmed().to_string(),
                    "steel" => t.white().to_string(),
                    "fairy" => t.magenta().to_string(),
                    "normal" => t.white().to_string(),
                    _ => t.to_string(),
                })
                .collect();
            let cat = match data.category.as_str() {
                "legendary" => "Legendary".red().bold().to_string(),
                "mythical" => "Mythical".magenta().bold().to_string(),
                "pseudo_legendary" => "Pseudo-Legendary".yellow().bold().to_string(),
                "starter" => "Starter".green().bold().to_string(),
                "starter_evolution" => "Starter Evo".green().to_string(),
                "rare" => "Rare".cyan().bold().to_string(),
                "baby" => "Baby".bright_magenta().to_string(),
                "uncommon" => "Uncommon".white().to_string(),
                "common" => "Common".bright_black().to_string(),
                _ => data.category.clone(),
            };
            (type_strings.join(" / "), data.power_rank as u32, cat)
        } else {
            ("???".to_string(), 0, "unknown".to_string())
        };

        total_power += power;

        let shiny_str = if entry.shiny {
            " [Shiny]".yellow().bold().to_string()
        } else {
            String::new()
        };

        println!(
            "  [{}] {}{}",
            format!("{:2}", i + 1).dimmed(),
            entry.name.green().bold(),
            shiny_str
        );
        println!(
            "      {} | Power: {} | {}",
            types_str,
            format!("{}", power).bright_yellow().bold(),
            cat_display
        );
    }

    println!();
    println!(
        "  {}",
        "──────────────────────────────────────".dimmed()
    );
    println!();
    println!(
        "  {} | Total Power: {}",
        format!("{}/20 slots", team.pokemon.len()).cyan(),
        format!("{}", total_power).bright_yellow().bold(),
    );
}
