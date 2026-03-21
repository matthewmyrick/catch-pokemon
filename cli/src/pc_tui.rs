use colored::*;
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::process::Command;

use crossterm::{
    cursor, terminal, ExecutableCommand,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
};

use crate::crypto::verify_chain;
use crate::display::{color_category, color_type, interactive_pokemon_search};
use crate::models::{
    BattleTeam, BattleTeamEntry, PcEntry, PcStorage, PokemonData, POKEMON_DATA, VALID_POKEMON,
};

pub fn show_pc(search: bool) {
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
    let mut cached_sprite_name = String::new();
    let mut cached_sprite: Vec<String> = Vec::new();
    let mut status_msg: Option<String> = None;
    let mut confirming_release = false;

    // Sort/filter state
    let sort_modes = ["true_odds", "power", "name", "count", "category"];
    let mut sort_idx: usize = 0; // default: true_odds (rarest first)

    let type_filters = ["all", "fire", "water", "grass", "electric", "ice", "fighting",
        "poison", "ground", "flying", "psychic", "bug", "rock", "ghost",
        "dragon", "dark", "steel", "fairy", "normal"];
    let mut type_filter_idx: usize = 0;

    let cat_filters = ["all", "common", "uncommon", "rare", "baby", "starter",
        "starter_evolution", "pseudo_legendary", "legendary", "mythical"];
    let mut cat_filter_idx: usize = 0;

    let mut searching = false;
    let mut search_term = String::new();

    // Precompute true odds for sorting
    let pokemon_db_sort: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();
    let valid_names_sort: std::collections::HashSet<&str> = VALID_POKEMON
        .lines().filter(|l| !l.is_empty()).collect();
    let total_weight: u32 = pokemon_db_sort.iter()
        .filter(|(n, _)| valid_names_sort.contains(n.as_str()))
        .map(|(_, d)| d.catch_rate as u32).sum();

    loop {
        let (tw, th) = terminal::size().unwrap_or((80, 24));
        let tw = tw as usize;
        let th = th as usize;
        let left_width = 28.min(tw / 3);
        let list_height = th.saturating_sub(5); // extra line for filter bar

        // Apply filters and search
        let filtered: Vec<usize> = (0..entries.len()).filter(|&i| {
            let e = &entries[i];
            // Type filter
            if type_filter_idx > 0 {
                let t = type_filters[type_filter_idx];
                if !e.types.iter().any(|et| et == t) { return false; }
            }
            // Category filter
            if cat_filter_idx > 0 {
                let c = cat_filters[cat_filter_idx];
                let normalized = e.name.replace("-", "_");
                if let Some(data) = pokemon_db_sort.get(&normalized) {
                    if data.category != c { return false; }
                } else { return false; }
            }
            // Search filter
            if !search_term.is_empty() {
                if !e.name.to_lowercase().contains(&search_term.to_lowercase()) { return false; }
            }
            true
        }).collect();

        // Sort filtered indices
        let mut sorted = filtered.clone();
        let sort_mode = sort_modes[sort_idx];
        sorted.sort_by(|&a, &b| {
            let ea = &entries[a];
            let eb = &entries[b];
            match sort_mode {
                "true_odds" => {
                    // Sort by true catch odds (rarest first)
                    let na = ea.name.replace("-", "_");
                    let nb = eb.name.replace("-", "_");
                    let odds_a = pokemon_db_sort.get(&na).map(|d| {
                        let cat_w: u32 = pokemon_db_sort.iter()
                            .filter(|(n, dd)| valid_names_sort.contains(n.as_str()) && dd.category == d.category)
                            .map(|(_, dd)| dd.catch_rate as u32).sum();
                        (cat_w as f64 / total_weight as f64) * (d.catch_rate as f64 / 255.0)
                    }).unwrap_or(0.0);
                    let odds_b = pokemon_db_sort.get(&nb).map(|d| {
                        let cat_w: u32 = pokemon_db_sort.iter()
                            .filter(|(n, dd)| valid_names_sort.contains(n.as_str()) && dd.category == d.category)
                            .map(|(_, dd)| dd.catch_rate as u32).sum();
                        (cat_w as f64 / total_weight as f64) * (d.catch_rate as f64 / 255.0)
                    }).unwrap_or(0.0);
                    odds_a.partial_cmp(&odds_b).unwrap_or(std::cmp::Ordering::Equal)
                }
                "power" => eb.power_rank.cmp(&ea.power_rank),
                "count" => eb.count.cmp(&ea.count),
                "category" => {
                    let cat_order = |name: &str| -> u8 {
                        let n = name.replace("-", "_");
                        match pokemon_db_sort.get(&n).map(|d| d.category.as_str()) {
                            Some("mythical") => 0, Some("legendary") => 1,
                            Some("pseudo_legendary") => 2, Some("starter_evolution") => 3,
                            Some("rare") => 4, Some("starter") => 5,
                            Some("uncommon") => 6, Some("baby") => 7,
                            Some("common") => 8, _ => 9,
                        }
                    };
                    cat_order(&ea.name).cmp(&cat_order(&eb.name)).then(ea.name.cmp(&eb.name))
                }
                _ => ea.name.cmp(&eb.name), // "name"
            }
        });

        // Clamp selected
        if sorted.is_empty() {
            selected = 0;
        } else if selected >= sorted.len() {
            selected = sorted.len() - 1;
        }

        // Get selected entry
        let sel_idx = if !sorted.is_empty() { sorted[selected] } else { 0 };
        let sel = if !entries.is_empty() { &entries[sel_idx] } else {
            // Empty — skip rendering
            break;
        };

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
        let type_label = if type_filter_idx > 0 { type_filters[type_filter_idx] } else { "all" };
        let cat_label = if cat_filter_idx > 0 { cat_filters[cat_filter_idx] } else { "all" };
        let header = format!(" {} ({}/{} shown | sort: {} | type: {} | cat: {})",
            "Pokemon PC".cyan().bold(),
            sorted.len().to_string().yellow(),
            entries.len().to_string().yellow(),
            sort_modes[sort_idx].cyan(),
            type_label.cyan(),
            cat_label.cyan());
        print!("{}\x1B[K\r\n", header);

        // Search bar
        if searching {
            print!(" {}: {}{}\x1B[K\r\n", "Search".cyan().bold(), search_term.yellow(), "▌".yellow());
        } else if !search_term.is_empty() {
            print!(" Search: {}\x1B[K\r\n", search_term.yellow());
        } else {
            print!(" {}\x1B[K\r\n", "─".repeat(tw.saturating_sub(2)).dimmed());
        }

        // Body rows
        let name_width = left_width.saturating_sub(8);
        for row in 0..list_height {
            let left = if row < sorted.len().saturating_sub(scroll_offset).min(list_height) {
                let idx = scroll_offset + row;
                if idx < sorted.len() {
                    let entry_idx = sorted[idx];
                    let e = &entries[entry_idx];
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
            if searching {
                print!(" {}\x1B[K",
                    "Type to filter | Esc: Stop search".dimmed());
            } else {
                let team_count = entries.iter().filter(|e| e.on_team).count();
                print!(" {}\x1B[K",
                    format!("↑↓ Nav | /: Search | S: Sort | F: Type | C: Cat | T: Team ({}/20) | R: Release | Q: Quit", team_count).dimmed());
            }
        }
        stdout().flush()?;

        // Drain queued events to prevent scroll/input lag
        while event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
            let _ = event::read();
        }

        // Input
        if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
            if searching {
                match code {
                    KeyCode::Esc | KeyCode::Enter => { searching = false; }
                    KeyCode::Backspace => { search_term.pop(); selected = 0; scroll_offset = 0; }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up => { if selected > 0 { selected -= 1; } }
                    KeyCode::Down => { if !sorted.is_empty() && selected < sorted.len() - 1 { selected += 1; } }
                    KeyCode::Char(c) => { search_term.push(c); selected = 0; scroll_offset = 0; }
                    _ => {}
                }
                continue;
            }
            match code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('/') => {
                    searching = true;
                    search_term.clear();
                    selected = 0;
                    scroll_offset = 0;
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    sort_idx = (sort_idx + 1) % sort_modes.len();
                    selected = 0;
                    scroll_offset = 0;
                    status_msg = Some(format!("Sort: {}", sort_modes[sort_idx]));
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    type_filter_idx = (type_filter_idx + 1) % type_filters.len();
                    selected = 0;
                    scroll_offset = 0;
                    status_msg = Some(format!("Type filter: {}", type_filters[type_filter_idx]));
                }
                KeyCode::Char('c') => {
                    cat_filter_idx = (cat_filter_idx + 1) % cat_filters.len();
                    selected = 0;
                    scroll_offset = 0;
                    status_msg = Some(format!("Category filter: {}", cat_filters[cat_filter_idx]));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if selected > 0 { selected -= 1; }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !sorted.is_empty() && selected < sorted.len() - 1 { selected += 1; }
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    if sorted.is_empty() { continue; }
                    let ei = sorted[selected];
                    let name = entries[ei].name.clone();
                    let normalized = name.to_lowercase().replace("-", "_");
                    let mut team = BattleTeam::load();
                    if team.pokemon.iter().any(|p| p.name.to_lowercase().replace("-", "_") == normalized) {
                        team.pokemon.retain(|p| p.name.to_lowercase().replace("-", "_") != normalized);
                        let _ = team.save();
                        entries[ei].on_team = false;
                    } else if team.pokemon.len() >= 20 {
                        status_msg = Some("Battle team is full! (20/20) Remove one first.".to_string());
                    } else {
                        let is_shiny = entries[ei].shiny_count > 0;
                        team.pokemon.push(BattleTeamEntry { name: name.to_lowercase(), shiny: is_shiny });
                        let _ = team.save();
                        entries[ei].on_team = true;
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if sorted.is_empty() { continue; }
                    let ei = sorted[selected];
                    let name = entries[ei].name.clone();
                    let count = entries[ei].count;

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
                    if sorted.is_empty() { confirming_release = false; continue; }
                    let ei = sorted[selected];
                    let name = entries[ei].name.clone();
                    confirming_release = false;

                    let mut storage = PcStorage::load();
                    let released = storage.release_pokemon(&name, 1);
                    if released > 0 {
                        if let Err(e) = storage.save() {
                            status_msg = Some(format!("Error saving: {}", e));
                        } else {
                            entries[ei].count -= 1;
                            if entries[ei].count == 0 {
                                entries.remove(ei);
                                if selected > 0 && selected >= sorted.len().saturating_sub(1) {
                                    selected = selected.saturating_sub(1);
                                }
                            }
                            let normalized = name.to_lowercase().replace("-", "_");
                            if !entries.iter().any(|e| e.name.to_lowercase().replace("-", "_") == normalized && e.count > 0) {
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
                    if !sorted.is_empty() { selected = sorted.len() - 1; }
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
