use colored::*;
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::process::Command;

use crossterm::{
    terminal,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
};

use crate::models::PcStorage;

pub fn color_type(t: &str) -> String {
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

pub fn color_category(cat: &str) -> String {
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

pub fn fuzzy_match(text: &str, pattern: &str) -> bool {
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

pub fn interactive_pokemon_search(storage: &PcStorage) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn show_pokemon_details(pokemon_name: &str, ball_counts: &HashMap<String, usize>, storage: &PcStorage) {
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
