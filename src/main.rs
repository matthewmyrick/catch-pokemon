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
#[command(author, version, about = "Catch Pokemon in your terminal!", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Try to catch a Pokemon
    Catch {
        /// Name of the Pokemon to catch
        pokemon: String,
        
        /// Type of Pokeball to use (pokeball, great, ultra, master)
        #[arg(short = 'b', long, default_value = "pokeball")]
        ball: String,
        
        /// Skip the animation
        #[arg(short = 's', long)]
        skip_animation: bool,
        
        /// Hide the Pokemon when it appears
        #[arg(long, default_value = "false")]
        hide_pokemon: bool,
    },
    
    /// Show all caught Pokemon in your PC
    Pc,
    
    /// Clear your PC storage (start fresh)
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
}

fn get_storage_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("catch-pokemon");
    path.push("pc_storage.json");
    path
}

fn load_pokeball_art(art_type: &str) -> Vec<String> {
    let art_path = format!("static/art/pokeball-{}.txt", art_type);
    if let Ok(content) = fs::read_to_string(&art_path) {
        content.lines().map(|line| line.to_string()).collect()
    } else {
        vec![]
    }
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
    let legendary = vec!["articuno", "zapdos", "moltres", "mewtwo", "mew", "lugia", 
                         "ho-oh", "celebi", "kyogre", "groudon", "rayquaza", "dialga", 
                         "palkia", "giratina", "arceus", "zekrom", "reshiram", "kyurem",
                         "xerneas", "yveltal", "zygarde", "solgaleo", "lunala", "necrozma"];
    
    let mythical = vec!["mew", "celebi", "jirachi", "deoxys", "manaphy", "darkrai", 
                        "shaymin", "arceus", "victini", "keldeo", "meloetta", "genesect",
                        "diancie", "hoopa", "volcanion", "magearna", "marshadow", "zeraora"];
    
    let starters = vec!["bulbasaur", "charmander", "squirtle", "chikorita", "cyndaquil", 
                        "totodile", "treecko", "torchic", "mudkip", "turtwig", "chimchar", 
                        "piplup", "snivy", "tepig", "oshawott", "chespin", "fennekin", 
                        "froakie", "rowlet", "litten", "popplio", "grookey", "scorbunny", "sobble"];
    
    let pseudo_legendary = vec!["dragonite", "tyranitar", "salamence", "metagross", 
                                "garchomp", "hydreigon", "goodra", "kommo-o", "dragapult"];
    
    let lower_name = pokemon_name.to_lowercase();
    
    if legendary.contains(&lower_name.as_str()) {
        3
    } else if mythical.contains(&lower_name.as_str()) {
        3
    } else if pseudo_legendary.contains(&lower_name.as_str()) {
        45
    } else if starters.contains(&lower_name.as_str()) {
        45
    } else if lower_name.contains("pikachu") {
        190
    } else if lower_name.contains("eevee") {
        45
    } else {
        let common = vec!["pidgey", "rattata", "caterpie", "weedle", "wurmple", 
                         "zigzagoon", "bidoof", "patrat", "bunnelby", "rookidee"];
        if common.contains(&lower_name.as_str()) {
            255
        } else {
            120
        }
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
        println!(
            "{}",
            format!("Oh no! The wild {} broke free and ran away!", pokemon).red()
        );
        println!("Try using a better Pokéball next time!");
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
        Commands::Clear => {
            clear_pc();
        }
    }
}