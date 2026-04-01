use chrono::Local;
use colored::*;
use hmac::Mac as HmacMac;
use rand::Rng;
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;

use crossterm::{cursor, terminal, ExecutableCommand};

use crate::crypto::{derive_signing_key, HmacSha256};
use crate::models::{
    PcStorage, Pokedex, PokeballType, PokemonData, POKEBALL_CAUGHT, POKEBALL_LEFT,
    POKEBALL_NOT_CAUGHT, POKEBALL_RIGHT, POKEBALL_STILL, POKEMON_DATA, VALID_POKEMON,
};

pub fn load_pokeball_art(art_type: &str) -> Vec<String> {
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

pub fn clear_lines(count: usize) {
    for _ in 0..count {
        stdout().execute(cursor::MoveUp(1)).unwrap();
        stdout()
            .execute(terminal::Clear(terminal::ClearType::CurrentLine))
            .unwrap();
    }
    stdout().flush().unwrap();
}

pub fn display_pokeball_art(lines: &[String]) {
    for line in lines {
        println!("{}", line);
    }
    stdout().flush().unwrap();
}

pub fn get_pokemon_catch_rate(pokemon_name: &str) -> u8 {
    // Parse the embedded Pokemon data once
    let pokemon_db: HashMap<String, PokemonData> = match serde_json::from_str(POKEMON_DATA) {
        Ok(data) => data,
        Err(_) => return 120, // Default catch rate if JSON parsing fails
    };

    // Normalize the Pokemon name to match our data format
    let normalized_name = pokemon_name
        .to_lowercase()
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
pub fn get_flee_rate(pokemon_name: &str) -> f32 {
    let pokemon_db: HashMap<String, PokemonData> = match serde_json::from_str(POKEMON_DATA) {
        Ok(data) => data,
        Err(_) => return 10.0,
    };

    let normalized_name = pokemon_name
        .to_lowercase()
        .replace("'", "")
        .replace(".", "")
        .replace(" ", "_")
        .replace("-", "_");

    match pokemon_db.get(&normalized_name) {
        Some(data) => data.flee_rate as f32,
        None => 10.0,
    }
}

pub fn calculate_catch_chance(pokemon_name: &str, ball: PokeballType) -> f32 {
    let base_catch_rate = get_pokemon_catch_rate(pokemon_name) as f32;
    let ball_modifier = ball.catch_modifier();

    let modified_rate = (base_catch_rate * ball_modifier).min(255.0);

    (modified_rate / 255.0 * 100.0).min(100.0)
}

pub fn throw_pokeball_animation(ball: PokeballType) {
    println!("You throw a {}!", ball.display_name());
    thread::sleep(Duration::from_millis(300));
}

pub fn wiggle_animation(wiggle_num: u8, ball: PokeballType, caught: bool) {
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

pub fn catch_pokemon(
    pokemon: String,
    skip_animation: bool,
    hide_pokemon: bool,
    shiny: bool,
    token: Option<String>,
    attempt: u32,
) {
    // Validate session token — prevents manual catching
    match &token {
        None => {
            println!(
                "{}",
                "You can't catch Pokemon directly! Use 'pokemon_encounter' first, then 'catch'."
                    .red()
                    .bold()
            );
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
                println!(
                    "{}",
                    "Session expired. Start a new encounter with 'pokemon_encounter'.".red()
                );
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
        format!("Throwing {} at {}!", ball.display_name(), pokemon)
            .cyan()
            .bold()
    );
    println!(
        "Catch chance: {}",
        format!("{:.1}%", catch_chance).bright_yellow().bold()
    );
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
                format!("Gotcha! {} was caught!", pokemon).green().bold()
            );
        }
        println!();

        let mut storage = PcStorage::load();
        storage.add_pokemon(pokemon.clone(), ball, shiny);
        if let Err(e) = storage.save() {
            eprintln!("{}", format!("SAVE FAILED: {}. Catch does NOT count!", e).red().bold());
            eprintln!("{}", "Your PC file may be corrupted. Run: catch-pokemon verify".red());
            return;
        } else {
            println!();
            if shiny {
                println!(
                    "{}",
                    format!("A shiny {} has been sent to your PC!", pokemon)
                        .yellow()
                        .bold()
                );
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
                "gives you a smug look",
            ];

            let action = actions[rng.gen_range(0..actions.len())];
            println!("{} {}.", pokemon, action);
        }
    }
}

pub fn encounter_pokemon(show_pokemon: bool) {
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
    let total_weight: u32 = pokemon_list
        .iter()
        .map(|(_, data)| data.catch_rate as u32)
        .sum();

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
    let mut mac =
        <HmacSha256 as HmacMac>::new_from_slice(&key).expect("HMAC accepts any key length");
    mac.update(token_data.as_bytes());
    let token = format!(
        "{}:{}",
        timestamp,
        hex::encode(mac.finalize().into_bytes())
    );

    // Print shiny status and token (for shell function)
    println!("Shiny: {}", is_shiny);
    println!("Token: {}", token);

    if show_pokemon {
        let mut args = vec!["-n", &display_name, "--no-title"];
        if is_shiny {
            args.push("-s");
        }

        let output = Command::new("pokemon-colorscripts").args(&args).output();

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
                let type_strings: Vec<String> = data
                    .types
                    .iter()
                    .map(|t| match t.as_str() {
                        "fire" => format!("{}", t.red().bold()),
                        "water" => format!("{}", t.blue().bold()),
                        "grass" => format!("{}", t.green().bold()),
                        "electric" => format!("{}", t.yellow().bold()),
                        "ice" => format!("{}", t.cyan().bold()),
                        "fighting" => format!("{}", t.red()),
                        "poison" => format!("{}", t.purple()),
                        "ground" => format!("{}", t.yellow()),
                        "flying" => format!("{}", t.cyan()),
                        "psychic" => format!("{}", t.magenta().bold()),
                        "bug" => format!("{}", t.green()),
                        "rock" => format!("{}", t.yellow().dimmed()),
                        "ghost" => format!("{}", t.purple().bold()),
                        "dragon" => format!("{}", t.blue().bold()),
                        "dark" => format!("{}", t.white().dimmed()),
                        "steel" => format!("{}", t.white()),
                        "fairy" => format!("{}", t.magenta()),
                        "normal" => format!("{}", t.white()),
                        _ => t.to_string(),
                    })
                    .collect();
                println!("Type: {}", type_strings.join(" / "));
            }

            let catch_pct = data.catch_rate as f32 / 255.0 * 100.0;
            println!(
                "Base catch rate: {}",
                format!("{:.1}%", catch_pct).bright_yellow().bold()
            );
        }
    }
}
