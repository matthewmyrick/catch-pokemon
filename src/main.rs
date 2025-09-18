use clap::{Parser, Subcommand};
use colored::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;
use chrono::{DateTime, Local};
use crossterm::{cursor, terminal, ExecutableCommand};

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
    #[command(long_about = "Attempt to catch a Pokemon using different types of Pokeballs.\n\n\
Each Pokemon has different catch rates based on rarity:\n\
- Legendary/Mythical: 3% base rate (very hard)\n\
- Pseudo-legendary/Starters: 45% base rate (hard)\n\
- Common Pokemon: 120-255% base rate (easy)\n\n\
Pokeball types and modifiers:\n\
- pokeball/poke: 1x modifier (red ball)\n\
- great/greatball: 1.5x modifier (blue ball)\n\
- ultra/ultraball: 2x modifier (yellow ball)\n\
- master/masterball: guaranteed catch (purple ball)\n\n\
Examples:\n\
  catch-pokemon catch pikachu\n\
  catch-pokemon catch mewtwo --ball master\n\
  catch-pokemon catch charizard --ball ultra --skip-animation\n\
  catch-pokemon catch bulbasaur --hide-pokemon")]
    Catch {
        /// Name of the Pokemon to catch (case insensitive)
        pokemon: String,
        
        /// Type of Pokeball to use: pokeball, great, ultra, master
        #[arg(short = 'b', long, default_value = "pokeball", 
              help = "Pokeball type (pokeball=1x, great=1.5x, ultra=2x, master=guaranteed)")]
        ball: String,
        
        /// Skip the animated Pokeball throwing sequence
        #[arg(short = 's', long, help = "Skip animations for faster catching")]
        skip_animation: bool,
        
        /// Hide the Pokemon ASCII art when it appears
        #[arg(long, default_value = "false", help = "Don't show Pokemon sprite, only catching animation")]
        hide_pokemon: bool,
    },
    
    /// Display your Pokemon collection with detailed statistics
    #[command(long_about = "View all Pokemon you've caught in your PC storage.\n\n\
Shows detailed information including:\n\
- Pokemon grouped by name with total counts\n\
- Breakdown of catches by ball type for each Pokemon\n\
- Summary statistics of total catches by ball type\n\
- Recent catch history with timestamps\n\n\
Example:\n\
  catch-pokemon pc")]
    Pc,
    
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
}

#[derive(Debug, Clone, Copy)]
enum PokeballType {
    Pokeball,
    GreatBall,
    UltraBall,
    MasterBall,
}

impl PokeballType {
    fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pokeball" | "poke" => Some(PokeballType::Pokeball),
            "great" | "greatball" => Some(PokeballType::GreatBall),
            "ultra" | "ultraball" => Some(PokeballType::UltraBall),
            "master" | "masterball" => Some(PokeballType::MasterBall),
            _ => None,
        }
    }
    
    fn catch_modifier(&self) -> f32 {
        match self {
            PokeballType::Pokeball => 1.0,
            PokeballType::GreatBall => 1.5,
            PokeballType::UltraBall => 2.0,
            PokeballType::MasterBall => 255.0,
        }
    }
    
    fn display_name(&self) -> &str {
        match self {
            PokeballType::Pokeball => "Poké Ball",
            PokeballType::GreatBall => "Great Ball",
            PokeballType::UltraBall => "Ultra Ball",
            PokeballType::MasterBall => "Master Ball",
        }
    }
    
    fn ball_symbol(&self) -> String {
        match self {
            PokeballType::Pokeball => "◓".red().to_string(),
            PokeballType::GreatBall => "◓".blue().to_string(),
            PokeballType::UltraBall => "◓".yellow().to_string(),
            PokeballType::MasterBall => "◓".magenta().to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct PokemonData {
    catch_rate: u8,
    category: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CaughtPokemon {
    name: String,
    caught_at: DateTime<Local>,
    ball_used: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct PcStorage {
    pokemon: Vec<CaughtPokemon>,
}

impl PcStorage {
    fn new() -> Self {
        PcStorage { pokemon: Vec::new() }
    }
    
    fn load() -> Self {
        let path = get_storage_path();
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(storage) = serde_json::from_str(&contents) {
                    return storage;
                }
            }
        }
        PcStorage::new()
    }
    
    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_storage_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self)?;
        fs::write(&path, json)?;
        Ok(())
    }
    
    fn add_pokemon(&mut self, name: String, ball: PokeballType) {
        self.pokemon.push(CaughtPokemon {
            name,
            caught_at: Local::now(),
            ball_used: ball.display_name().to_string(),
        });
    }
    
    fn release_pokemon(&mut self, name: &str, count: usize) -> usize {
        let mut released = 0;
        
        self.pokemon.retain(|p| {
            if p.name.to_lowercase() == name.to_lowercase() && released < count {
                released += 1;
                false // Remove this Pokemon
            } else {
                true // Keep this Pokemon
            }
        });
        
        released
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

// Embed the art files directly in the binary
const POKEBALL_STILL: &str = include_str!("../static/art/pokeball-still.txt");
const POKEBALL_LEFT: &str = include_str!("../static/art/pokeball-left.txt");
const POKEBALL_RIGHT: &str = include_str!("../static/art/pokeball-right.txt");
const POKEBALL_CAUGHT: &str = include_str!("../static/art/pokeball-caught.txt");
const POKEBALL_NOT_CAUGHT: &str = include_str!("../static/art/pokeball-not-caught.txt");

// Embed the Pokemon data directly in the binary
const POKEMON_DATA: &str = include_str!("../data/pokemon.json");

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


fn catch_pokemon(pokemon: String, ball_str: String, skip_animation: bool, hide_pokemon: bool) {
    let ball = match PokeballType::from_string(&ball_str) {
        Some(b) => b,
        None => {
            println!("{}", format!("Invalid ball type: {}. Use pokeball, great, ultra, or master", ball_str).red());
            return;
        }
    };
    
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
        println!(
            "{}",
            format!("Gotcha! {} was caught!", pokemon)
                .green()
                .bold()
        );
        println!();

        let mut storage = PcStorage::load();
        storage.add_pokemon(pokemon.clone(), ball);
        if let Err(e) = storage.save() {
            eprintln!("Warning: Could not save to PC: {}", e);
        } else {
            println!();
            println!("{} has been sent to your PC!", pokemon.cyan());
        }

    } else {
        // 10% chance the Pokemon runs away, 90% chance it just breaks free
        let run_away_chance = rng.gen_range(0.0..100.0);
        if run_away_chance < 10.0 {
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

fn show_pc() {
    let storage = PcStorage::load();
    
    if storage.pokemon.is_empty() {
        println!("{}", "Your PC is empty. Go catch some Pokemon!".yellow());
        return;
    }
    
    println!("{}", "╔══════════════════════════════════════════════╗".cyan());
    println!("{}", "║            Pokemon PC Storage                ║".cyan().bold());
    println!("{}", "╠══════════════════════════════════════════════╣".cyan());
    
    // Create nested HashMap: Pokemon name -> Ball type -> Count
    let mut pokemon_ball_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for p in &storage.pokemon {
        pokemon_ball_counts
            .entry(p.name.clone())
            .or_insert_with(HashMap::new)
            .entry(p.ball_used.clone())
            .and_modify(|e| *e += 1)
            .or_insert(1);
    }
    
    let mut sorted_pokemon: Vec<_> = pokemon_ball_counts.iter().collect();
    sorted_pokemon.sort_by(|a, b| a.0.cmp(b.0));
    
    for (name, ball_counts) in sorted_pokemon {
        let total_count: usize = ball_counts.values().sum();
        
        if total_count > 1 {
            println!("║ • {} (x{}):", name.green().bold(), total_count.to_string().yellow());
            for (ball, count) in ball_counts {
                println!("║   └─ {} with {}", 
                        format!("x{}", count).cyan(), 
                        ball.magenta());
            }
        } else {
            let (ball, _) = ball_counts.iter().next().unwrap();
            println!("║ • {} (caught with {})", name.green(), ball.magenta());
        }
    }
    
    println!("{}", "╠══════════════════════════════════════════════╣".cyan());
    println!("║ Total Pokemon caught: {:<22} ║", storage.pokemon.len().to_string().yellow().bold());
    
    // Ball type summary
    let mut ball_summary: HashMap<String, usize> = HashMap::new();
    for p in &storage.pokemon {
        *ball_summary.entry(p.ball_used.clone()).or_insert(0) += 1;
    }
    
    println!("║                                              ║");
    println!("║ Catches by ball type:                        ║");
    for (ball, count) in ball_summary {
        println!("║   • {}: {:<31} ║", ball.magenta(), count.to_string().cyan());
    }
    
    println!("{}", "╚══════════════════════════════════════════════╝".cyan());
    
    println!();
    println!("Recent catches:");
    for pokemon in storage.pokemon.iter().rev().take(5) {
        println!("  • {} caught with {} at {}", 
                pokemon.name.green(), 
                pokemon.ball_used.cyan(),
                pokemon.caught_at.format("%Y-%m-%d %H:%M"));
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

fn main() {
    let args = Args::parse();
    
    match args.command {
        Commands::Catch { pokemon, ball, skip_animation, hide_pokemon } => {
            catch_pokemon(pokemon, ball, skip_animation, hide_pokemon);
        },
        Commands::Pc => {
            show_pc();
        },
        Commands::Release { pokemon, number } => {
            release_pokemon(pokemon, number);
        },
        Commands::Status { pokemon, boolean } => {
            check_pokemon(pokemon, boolean);
        },
        Commands::Clear => {
            clear_pc();
        }
    }
}