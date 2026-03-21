use colored::*;
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::process::Command;

use crossterm::{
    cursor, terminal, ExecutableCommand,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
};

use crate::display::{color_category, color_type};
use crate::models::{PcStorage, Pokedex, PokemonData, POKEMON_DATA, VALID_POKEMON};

pub fn show_pokedex() {
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
