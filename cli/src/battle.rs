use colored::*;
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::time::Duration;

use crossterm::{
    cursor, terminal, ExecutableCommand,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
};

use crate::api::{api_get, api_post, get_api_url, get_github_token};
use crate::display::color_type;
use crate::models::{BattleTeam, PcStorage, PokemonData, POKEMON_DATA};

// --- Data types ---

struct BattlePokemon {
    name: String,
    types: Vec<String>,
    power_rank: u8,
    shiny: bool,
}

#[derive(PartialEq)]
enum BattleState {
    TeamSelection,
    WaitingForOpponent,
    RoundResults,
    BattleComplete,
}

#[derive(PartialEq)]
enum Pane {
    Left,
    Right,
}

struct BattleContext {
    state: BattleState,
    battle_id: String,
    token: String,
    user_id: String,
    opponent_id: String,
    our_team: Vec<BattlePokemon>,
    opponent_pc: Vec<BattlePokemon>,
    active_pane: Pane,
    left_selected: usize,
    left_scroll: usize,
    right_selected: usize,
    right_scroll: usize,
    chosen: Vec<bool>,
    current_round: usize,
    our_wins: u64,
    opp_wins: u64,
    last_our_score: f64,
    last_opp_score: f64,
    last_round_won: bool,
    result_message: String,
    spinner_frame: usize,
    status_msg: Option<String>,
    confirming_forfeit: bool,
    poll_failures: usize,
}

// --- Auth handshake + matchmaking (unchanged) ---

pub fn battle_tui() {
    println!();
    println!("{}", "========================================".cyan().bold());
    println!("{}", "         POKEMON BATTLE ARENA           ".cyan().bold());
    println!("{}", "========================================".cyan().bold());
    println!();

    // Step 1: Get GitHub token
    println!("{}", "[1/5] Authenticating with GitHub...".dimmed());
    let token = match get_github_token() {
        Some(t) => {
            println!("  {} GitHub token found", "OK".green().bold());
            t
        }
        None => {
            eprintln!("  {} Not logged in to GitHub", "FAIL".red().bold());
            eprintln!("  Run: {}", "gh auth login".yellow());
            return;
        }
    };

    // Step 2: Connect to battle server
    println!("{}", "[2/5] Connecting to battle server...".dimmed());
    let server_url = get_api_url();
    match api_get("/health", &token) {
        Some(s) => {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                let db_status = data["database"].as_str().unwrap_or("unknown");
                println!("  {} Server at {}", "OK".green().bold(), server_url.cyan());
                println!("  {} Database: {}", "OK".green().bold(), db_status.green());
            } else {
                println!("  {} Server at {}", "OK".green().bold(), server_url.cyan());
            }
        }
        None => {
            eprintln!("  {} Could not connect to {}", "FAIL".red().bold(), server_url);
            return;
        }
    }

    // Step 3: Authenticate user
    println!("{}", "[3/5] Verifying trainer identity...".dimmed());
    let user_id = match api_get("/api/me", &token) {
        Some(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(data) => {
                let uid = data["user_id"].as_str().unwrap_or("unknown").to_string();
                println!("  {} Authenticated as {}", "OK".green().bold(), uid.cyan().bold());
                uid
            }
            Err(_) => {
                eprintln!("  {} Server returned invalid response", "FAIL".red().bold());
                return;
            }
        },
        None => {
            eprintln!("  {} Authentication failed", "FAIL".red().bold());
            return;
        }
    };

    // Step 4: Load PC + battle team
    println!("{}", "[4/5] Loading battle data...".dimmed());
    let storage = PcStorage::load();
    if storage.pokemon.len() < 6 {
        eprintln!("  {} You have {} Pokemon — need at least 6", "FAIL".red().bold(), storage.pokemon.len());
        return;
    }
    println!("  {} PC loaded ({} Pokemon)", "OK".green().bold(), storage.pokemon.len());

    let team = BattleTeam::load();
    if team.pokemon.is_empty() {
        eprintln!("  {} Battle team is empty", "FAIL".red().bold());
        eprintln!("  Add Pokemon with: {}", "catch-pokemon team --add <name>".yellow());
        return;
    }
    println!("  {} Battle team loaded ({} Pokemon)", "OK".green().bold(), team.pokemon.len());

    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();

    // Build PC data for matchmaking
    let pc_pokemon: Vec<serde_json::Value> = storage.pokemon.iter().map(|p| {
        let normalized = p.name.replace("-", "_");
        let (types, power) = pokemon_db.get(&normalized)
            .map(|d| (d.types.clone(), d.power_rank))
            .unwrap_or((vec![], 0));
        serde_json::json!({
            "name": p.name, "types": types, "power_rank": power, "shiny": p.shiny
        })
    }).collect();

    if pc_pokemon.len() < 6 {
        eprintln!("  {} Not enough valid Pokemon", "FAIL".red().bold());
        return;
    }

    // Step 5: Matchmaking
    println!("{}", "[5/5] Searching for opponent...".dimmed());
    println!("  Waiting up to 60 seconds for a match...");

    let body = serde_json::json!({ "pc": pc_pokemon }).to_string();
    let match_data: serde_json::Value = match api_post("/api/battle/join", &token, &body) {
        Some(s) => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(_) => { eprintln!("  {}", "Invalid response from server.".red()); return; }
        },
        None => { eprintln!("  {}", "No opponent found or connection lost.".red()); return; }
    };

    let status = match_data["status"].as_str().unwrap_or("");
    if status == "timeout" {
        println!("  {}", "No opponent found. Try again later.".yellow());
        return;
    }
    if status != "matched" {
        let msg = match_data["error"].as_str().or(match_data["message"].as_str()).unwrap_or("Unknown error");
        eprintln!("  {}", msg.red());
        return;
    }

    let battle_id = match_data["battle_id"].as_str().unwrap_or("").to_string();
    let opponent_id = match_data["opponent_id"].as_str().unwrap_or("???").to_string();

    println!("  {} Matched against {}", "OK".green().bold(), opponent_id.magenta().bold());

    // Build our team from BattleTeam
    let our_team: Vec<BattlePokemon> = team.pokemon.iter().map(|p| {
        let normalized = p.name.replace("-", "_");
        let (types, power) = pokemon_db.get(&normalized)
            .map(|d| (d.types.clone(), d.power_rank))
            .unwrap_or((vec![], 0));
        BattlePokemon { name: p.name.clone(), types, power_rank: power, shiny: p.shiny }
    }).collect();

    // Build opponent PC
    let opponent_pc: Vec<BattlePokemon> = match_data["opponent_pc"].as_array()
        .map(|arr| arr.iter().map(|p| {
            BattlePokemon {
                name: p["name"].as_str().unwrap_or("???").to_string(),
                types: p["types"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                power_rank: p["power_rank"].as_u64().unwrap_or(0) as u8,
                shiny: p["shiny"].as_bool().unwrap_or(false),
            }
        }).collect())
        .unwrap_or_default();

    let chosen = vec![false; our_team.len()];

    let mut ctx = BattleContext {
        state: BattleState::TeamSelection,
        battle_id, token, user_id, opponent_id,
        our_team, opponent_pc,
        active_pane: Pane::Left,
        left_selected: 0, left_scroll: 0,
        right_selected: 0, right_scroll: 0,
        chosen,
        current_round: 1,
        our_wins: 0, opp_wins: 0,
        last_our_score: 0.0, last_opp_score: 0.0,
        last_round_won: false,
        result_message: String::new(),
        spinner_frame: 0,
        status_msg: None,
        confirming_forfeit: false,
        poll_failures: 0,
    };

    if let Err(e) = run_battle_tui(&mut ctx) {
        eprintln!("Battle TUI error: {}", e);
    }
}

// --- Main TUI loop ---

fn run_battle_tui(ctx: &mut BattleContext) -> Result<(), Box<dyn std::error::Error>> {
    stdout().execute(crossterm::terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    stdout().execute(cursor::Hide)?;

    let result = battle_loop(ctx);

    stdout().execute(cursor::Show)?;
    terminal::disable_raw_mode()?;
    stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

    result
}

fn battle_loop(ctx: &mut BattleContext) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match ctx.state {
            BattleState::TeamSelection => {
                render_selection(ctx)?;
                if handle_selection_input(ctx)? { break; }
            }
            BattleState::WaitingForOpponent => {
                render_waiting(ctx)?;
                if handle_waiting_input(ctx)? { break; }
            }
            BattleState::RoundResults => {
                render_results(ctx)?;
                if handle_results_input(ctx)? { break; }
            }
            BattleState::BattleComplete => {
                render_complete(ctx)?;
                // Wait for any key then exit
                loop {
                    if let Ok(Event::Key(_)) = event::read() { break; }
                }
                break;
            }
        }
    }
    Ok(())
}

// --- Helpers ---

fn format_pokemon_cell(p: &BattlePokemon, width: usize, highlight: bool) -> String {
    let _types_str = p.types.iter().map(|t| color_type(t)).collect::<Vec<_>>().join("/");
    let shiny = if p.shiny { "\x1B[1;33m*\x1B[0m" } else { " " };
    let name_w = width.saturating_sub(16);
    let truncated: String = p.name.chars().take(name_w).collect();
    let padded = format!("{:<w$}", truncated, w = name_w);

    if highlight {
        format!("\x1B[7m {}{} P:{:<3}\x1B[0m", padded, shiny, p.power_rank)
    } else {
        format!(" \x1B[32m{}\x1B[0m{} \x1B[33mP:{:<3}\x1B[0m", padded, shiny, p.power_rank)
    }
}

fn selected_count(ctx: &BattleContext) -> usize {
    ctx.chosen.iter().filter(|&&c| c).count()
}

// --- Team Selection ---

fn render_selection(ctx: &BattleContext) -> Result<(), Box<dyn std::error::Error>> {
    let (tw, th) = terminal::size()?;
    let tw = tw as usize;
    let th = th as usize;
    let left_width = 36.min(tw / 2);
    let list_height = th.saturating_sub(6);
    let count = selected_count(ctx);

    stdout().execute(cursor::MoveTo(0, 0))?;

    // Header
    let round_str = format!(" ROUND {} — Select 6 Pokemon", ctx.current_round);
    let count_str = format!("Selected: {}/6 ", count);
    let padding = tw.saturating_sub(round_str.len() + count_str.len());
    let count_color = if count == 6 { "\x1B[1;32m" } else { "\x1B[1;33m" };
    print!("\x1B[1;36m{}\x1B[0m{}{}{}\x1B[0m\x1B[K\r\n", round_str, " ".repeat(padding), count_color, count_str);

    // Column headers
    let left_header = if ctx.active_pane == Pane::Left { "\x1B[1;36m YOUR TEAM\x1B[0m" } else { "\x1B[90m YOUR TEAM\x1B[0m" };
    let right_header = if ctx.active_pane == Pane::Right { "\x1B[1;36m OPPONENT'S POKEMON\x1B[0m" } else { "\x1B[90m OPPONENT'S POKEMON\x1B[0m" };
    print!(" {}\x1B[{}G\x1B[90m│\x1B[0m{}\x1B[K\r\n", left_header, left_width + 1, right_header);

    // Separator
    print!(" {}\x1B[K\r\n", "\x1B[90m─\x1B[0m".repeat(tw.saturating_sub(2)));

    // Body
    let left_name_w = left_width.saturating_sub(12);
    let right_name_w = (tw.saturating_sub(left_width + 2)).saturating_sub(16);

    for row in 0..list_height {
        // Left pane
        let left_idx = ctx.left_scroll + row;
        let left_cell = if left_idx < ctx.our_team.len() {
            let p = &ctx.our_team[left_idx];
            let check = if ctx.chosen[left_idx] { "\x1B[1;32m[x]\x1B[0m" } else { "\x1B[90m[ ]\x1B[0m" };
            let arrow = if left_idx == ctx.left_selected && ctx.active_pane == Pane::Left { ">" } else { " " };
            let shiny = if p.shiny { "\x1B[1;33m*\x1B[0m" } else { " " };
            let truncated: String = p.name.chars().take(left_name_w).collect();
            let padded = format!("{:<w$}", truncated, w = left_name_w);

            if left_idx == ctx.left_selected && ctx.active_pane == Pane::Left {
                format!(" {}{}\x1B[7m {}{} P:{:<3}\x1B[0m", check, arrow, padded, shiny, p.power_rank)
            } else {
                format!(" {}{} \x1B[32m{}\x1B[0m{} \x1B[33mP:{:<3}\x1B[0m", check, arrow, padded, shiny, p.power_rank)
            }
        } else {
            format!("{:<w$}", "", w = left_width)
        };

        // Right pane
        let right_idx = ctx.right_scroll + row;
        let right_cell = if right_idx < ctx.opponent_pc.len() {
            let p = &ctx.opponent_pc[right_idx];
            let arrow = if right_idx == ctx.right_selected && ctx.active_pane == Pane::Right { ">" } else { " " };
            let shiny = if p.shiny { "\x1B[1;33m*\x1B[0m" } else { " " };
            let truncated: String = p.name.chars().take(right_name_w).collect();
            let padded = format!("{:<w$}", truncated, w = right_name_w);

            if right_idx == ctx.right_selected && ctx.active_pane == Pane::Right {
                format!(" {}\x1B[7m {}{} P:{:<3}\x1B[0m", arrow, padded, shiny, p.power_rank)
            } else {
                format!(" {} \x1B[32m{}\x1B[0m{} \x1B[33mP:{:<3}\x1B[0m", arrow, padded, shiny, p.power_rank)
            }
        } else {
            String::new()
        };

        print!("{}\x1B[{}G\x1B[90m│\x1B[0m{}\x1B[K\r\n", left_cell, left_width + 1, right_cell);
    }

    // Footer separator
    print!(" {}\x1B[K\r\n", "\x1B[90m─\x1B[0m".repeat(tw.saturating_sub(2)));

    // Footer
    if let Some(ref msg) = ctx.status_msg {
        print!(" \x1B[1;31m{}\x1B[0m\x1B[K", msg);
    } else if ctx.confirming_forfeit {
        print!(" \x1B[1;31mForfeit the battle? Press Y to confirm, any other key to cancel\x1B[0m\x1B[K");
    } else {
        print!(" \x1B[90mTab: Switch pane | ↑↓/jk: Nav | Space: Toggle | Enter: Lock in | q: Forfeit\x1B[0m\x1B[K");
    }
    stdout().flush()?;
    Ok(())
}

fn handle_selection_input(ctx: &mut BattleContext) -> Result<bool, Box<dyn std::error::Error>> {
    let (_, th) = terminal::size()?;
    let list_height = (th as usize).saturating_sub(6);
    ctx.status_msg = None;

    // Drain queued events
    while event::poll(Duration::from_millis(0))? {
        let _ = event::read();
    }

    if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
        if ctx.confirming_forfeit {
            ctx.confirming_forfeit = false;
            if code == KeyCode::Char('y') || code == KeyCode::Char('Y') {
                return Ok(true); // exit battle
            }
            return Ok(false);
        }

        match code {
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                ctx.confirming_forfeit = true;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                ctx.active_pane = if ctx.active_pane == Pane::Left { Pane::Right } else { Pane::Left };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if ctx.active_pane == Pane::Left {
                    if ctx.left_selected > 0 { ctx.left_selected -= 1; }
                    if ctx.left_selected < ctx.left_scroll { ctx.left_scroll = ctx.left_selected; }
                } else {
                    if ctx.right_selected > 0 { ctx.right_selected -= 1; }
                    if ctx.right_selected < ctx.right_scroll { ctx.right_scroll = ctx.right_selected; }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if ctx.active_pane == Pane::Left {
                    if ctx.left_selected + 1 < ctx.our_team.len() { ctx.left_selected += 1; }
                    if ctx.left_selected >= ctx.left_scroll + list_height { ctx.left_scroll = ctx.left_selected + 1 - list_height; }
                } else {
                    if ctx.right_selected + 1 < ctx.opponent_pc.len() { ctx.right_selected += 1; }
                    if ctx.right_selected >= ctx.right_scroll + list_height { ctx.right_scroll = ctx.right_selected + 1 - list_height; }
                }
            }
            KeyCode::Home => {
                if ctx.active_pane == Pane::Left { ctx.left_selected = 0; ctx.left_scroll = 0; }
                else { ctx.right_selected = 0; ctx.right_scroll = 0; }
            }
            KeyCode::End => {
                if ctx.active_pane == Pane::Left {
                    ctx.left_selected = ctx.our_team.len().saturating_sub(1);
                    if ctx.left_selected >= list_height { ctx.left_scroll = ctx.left_selected + 1 - list_height; }
                } else {
                    ctx.right_selected = ctx.opponent_pc.len().saturating_sub(1);
                    if ctx.right_selected >= list_height { ctx.right_scroll = ctx.right_selected + 1 - list_height; }
                }
            }
            KeyCode::Char(' ') => {
                if ctx.active_pane == Pane::Left && ctx.left_selected < ctx.chosen.len() {
                    if ctx.chosen[ctx.left_selected] {
                        ctx.chosen[ctx.left_selected] = false;
                    } else if selected_count(ctx) < 6 {
                        ctx.chosen[ctx.left_selected] = true;
                    } else {
                        ctx.status_msg = Some("Already selected 6! Deselect one first.".to_string());
                    }
                }
            }
            KeyCode::Enter => {
                let count = selected_count(ctx);
                if count != 6 {
                    ctx.status_msg = Some(format!("Select exactly 6 Pokemon ({}/6 selected)", count));
                } else {
                    // Build selected team and submit
                    let selected_team: Vec<serde_json::Value> = ctx.chosen.iter().enumerate()
                        .filter(|(_, &c)| c)
                        .map(|(i, _)| {
                            let p = &ctx.our_team[i];
                            serde_json::json!({
                                "name": p.name, "types": p.types, "power_rank": p.power_rank, "shiny": p.shiny
                            })
                        })
                        .collect();

                    let body = serde_json::json!({
                        "battle_id": ctx.battle_id,
                        "team": selected_team
                    }).to_string();

                    api_post("/api/battle/select", &ctx.token, &body);
                    ctx.state = BattleState::WaitingForOpponent;
                    ctx.spinner_frame = 0;
                    ctx.poll_failures = 0;
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

// --- Waiting for Opponent ---

fn render_waiting(ctx: &BattleContext) -> Result<(), Box<dyn std::error::Error>> {
    let (tw, th) = terminal::size()?;
    let tw = tw as usize;
    let th = th as usize;
    let left_width = 36.min(tw / 2);
    let list_height = th.saturating_sub(6);
    let spinners = ["   ", ".  ", ".. ", "..."];
    let spinner = spinners[ctx.spinner_frame % spinners.len()];

    stdout().execute(cursor::MoveTo(0, 0))?;

    // Header
    print!("\x1B[1;36m ROUND {} — Waiting for opponent{}\x1B[0m\x1B[K\r\n", ctx.current_round, spinner);

    // Column headers
    print!(" \x1B[1;36m YOUR TEAM\x1B[0m\x1B[{}G\x1B[90m│\x1B[0m\x1B[K\r\n", left_width + 1);

    // Separator
    print!(" {}\x1B[K\r\n", "\x1B[90m─\x1B[0m".repeat(tw.saturating_sub(2)));

    // Body — show our locked-in team on left, waiting message on right
    let selected_pokemon: Vec<&BattlePokemon> = ctx.chosen.iter().enumerate()
        .filter(|(_, &c)| c)
        .map(|(i, _)| &ctx.our_team[i])
        .collect();

    for row in 0..list_height {
        let left_cell = if row < selected_pokemon.len() {
            let p = selected_pokemon[row];
            format_pokemon_cell(p, left_width, false)
        } else {
            format!("{:<w$}", "", w = left_width)
        };

        let right_cell = if row == list_height / 2 - 1 {
            "\x1B[1;33m   Waiting for opponent\x1B[0m".to_string()
        } else if row == list_height / 2 {
            format!("\x1B[33m          {}\x1B[0m", spinner)
        } else {
            String::new()
        };

        print!("{}\x1B[{}G\x1B[90m│\x1B[0m{}\x1B[K\r\n", left_cell, left_width + 1, right_cell);
    }

    // Footer
    print!(" {}\x1B[K\r\n", "\x1B[90m─\x1B[0m".repeat(tw.saturating_sub(2)));
    print!(" \x1B[90mq: Forfeit\x1B[0m\x1B[K");
    stdout().flush()?;
    Ok(())
}

fn handle_waiting_input(ctx: &mut BattleContext) -> Result<bool, Box<dyn std::error::Error>> {
    // Use poll with timeout — check for keys, poll API on timeout
    if event::poll(Duration::from_millis(2000))? {
        if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
            match code {
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
                KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
                _ => {}
            }
        }
    } else {
        // Timeout — poll API for status
        ctx.spinner_frame += 1;

        if let Some(s) = api_get("/api/battle/status", &ctx.token) {
            ctx.poll_failures = 0;
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                let bstatus = data["status"].as_str().unwrap_or("");
                let rounds = data["rounds"].as_array().map(|a| a.len()).unwrap_or(0);

                if rounds >= ctx.current_round {
                    // Round complete — parse results
                    if let Some(r) = data["rounds"].as_array().and_then(|a| a.last()) {
                        let res = &r["result"];
                        let me = data["you"].as_str().unwrap_or("");
                        let winner = res["winner"].as_str().unwrap_or("");

                        ctx.last_our_score = res["p1_score"].as_f64().unwrap_or(0.0);
                        ctx.last_opp_score = res["p2_score"].as_f64().unwrap_or(0.0);

                        // Swap scores if we're player 2
                        if me != data.get("battle_id").and_then(|_| Some("")).unwrap_or("") {
                            // Use the "you" field to determine perspective
                            if me == data.get("opponent").and_then(|v| v.as_str()).unwrap_or("") {
                                // We're actually player 2, scores might be swapped
                            }
                        }

                        ctx.last_round_won = winner == me;
                        ctx.our_wins = data["p1_wins"].as_u64().unwrap_or(0);
                        ctx.opp_wins = data["p2_wins"].as_u64().unwrap_or(0);

                        // Determine correct wins perspective
                        let p1 = data.get("you").and_then(|v| v.as_str()).unwrap_or("");
                        let _opponent_field = data.get("opponent").and_then(|v| v.as_str()).unwrap_or("");
                        if !p1.is_empty() && p1 == ctx.user_id {
                            // We are p1, wins are correct
                        } else {
                            // We are p2, swap
                            std::mem::swap(&mut ctx.our_wins, &mut ctx.opp_wins);
                        }
                    }

                    ctx.state = BattleState::RoundResults;
                    return Ok(false);
                }

                if bstatus == "complete" || bstatus == "abandoned" || bstatus == "none" {
                    let msg = data["message"].as_str().unwrap_or("Battle ended.");
                    ctx.result_message = msg.to_string();
                    ctx.state = BattleState::BattleComplete;
                    return Ok(false);
                }
            }
        } else {
            ctx.poll_failures += 1;
            if ctx.poll_failures > 15 {
                ctx.result_message = "Connection lost.".to_string();
                ctx.state = BattleState::BattleComplete;
                return Ok(false);
            }
        }
    }
    Ok(false)
}

// --- Round Results ---

fn render_results(ctx: &BattleContext) -> Result<(), Box<dyn std::error::Error>> {
    let (tw, th) = terminal::size()?;
    let tw = tw as usize;
    let th = th as usize;
    let left_width = 36.min(tw / 2);
    let list_height = th.saturating_sub(6);

    stdout().execute(cursor::MoveTo(0, 0))?;

    // Header
    let series = format!("Series: {}-{}", ctx.our_wins, ctx.opp_wins);
    let header = format!(" ROUND {} RESULTS", ctx.current_round);
    let padding = tw.saturating_sub(header.len() + series.len() + 1);
    print!("\x1B[1;36m{}\x1B[0m{}\x1B[1;33m{}\x1B[0m\x1B[K\r\n", header, " ".repeat(padding), series);

    // Column headers
    print!(" \x1B[1;36m YOUR TEAM\x1B[0m\x1B[{}G\x1B[90m│\x1B[0m \x1B[1;35m OPPONENT'S TEAM\x1B[0m\x1B[K\r\n", left_width + 1);

    // Separator
    print!(" {}\x1B[K\r\n", "\x1B[90m─\x1B[0m".repeat(tw.saturating_sub(2)));

    // Body — show both teams
    let our_selected: Vec<&BattlePokemon> = ctx.chosen.iter().enumerate()
        .filter(|(_, &c)| c)
        .map(|(i, _)| &ctx.our_team[i])
        .collect();

    let result_row = 8; // Row to show scores
    let winner_row = 11; // Row for win/loss message

    for row in 0..list_height {
        let left_cell = if row < our_selected.len() {
            format_pokemon_cell(our_selected[row], left_width, false)
        } else if row == result_row {
            format!(" \x1B[1mYour Score:     {:.3}\x1B[0m", ctx.last_our_score)
        } else if row == winner_row {
            if ctx.last_round_won {
                " \x1B[1;7;32m  YOU WON THIS ROUND!  \x1B[0m".to_string()
            } else {
                " \x1B[1;7;31m  YOU LOST THIS ROUND  \x1B[0m".to_string()
            }
        } else {
            format!("{:<w$}", "", w = left_width)
        };

        let right_cell = if row < 6 {
            // Show opponent team from last round results (we don't have separate storage, show from their PC)
            // The API doesn't return opponent's selected team in the status response clearly,
            // so we show their full PC as reference
            if row < ctx.opponent_pc.len() {
                format_pokemon_cell(&ctx.opponent_pc[row], tw.saturating_sub(left_width + 2), false)
            } else {
                String::new()
            }
        } else if row == result_row {
            format!(" \x1B[1mOpponent Score: {:.3}\x1B[0m", ctx.last_opp_score)
        } else {
            String::new()
        };

        print!("{}\x1B[{}G\x1B[90m│\x1B[0m{}\x1B[K\r\n", left_cell, left_width + 1, right_cell);
    }

    // Footer
    print!(" {}\x1B[K\r\n", "\x1B[90m─\x1B[0m".repeat(tw.saturating_sub(2)));
    print!(" \x1B[90mPress any key to continue\x1B[0m\x1B[K");
    stdout().flush()?;
    Ok(())
}

fn handle_results_input(ctx: &mut BattleContext) -> Result<bool, Box<dyn std::error::Error>> {
    // Drain events
    while event::poll(Duration::from_millis(0))? {
        let _ = event::read();
    }

    // Wait for any key
    if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }

        // Check if battle is over
        if ctx.our_wins >= 3 || ctx.opp_wins >= 3 {
            if ctx.our_wins >= 3 {
                ctx.result_message = format!("YOU WIN! Final score: {}-{}", ctx.our_wins, ctx.opp_wins);
            } else {
                ctx.result_message = format!("YOU LOSE. Final score: {}-{}", ctx.our_wins, ctx.opp_wins);
            }
            ctx.state = BattleState::BattleComplete;
        } else {
            // Next round — reset selection
            ctx.current_round += 1;
            ctx.chosen = vec![false; ctx.our_team.len()];
            ctx.left_selected = 0;
            ctx.left_scroll = 0;
            ctx.right_selected = 0;
            ctx.right_scroll = 0;
            ctx.active_pane = Pane::Left;
            ctx.state = BattleState::TeamSelection;
        }
    }
    Ok(false)
}

// --- Battle Complete ---

fn render_complete(ctx: &BattleContext) -> Result<(), Box<dyn std::error::Error>> {
    let (tw, th) = terminal::size()?;
    let tw = tw as usize;
    let th = th as usize;

    stdout().execute(cursor::MoveTo(0, 0))?;

    let won = ctx.our_wins >= 3;

    for row in 0..th {
        if row == th / 2 - 3 {
            let line = "═".repeat(tw.saturating_sub(2));
            print!(" \x1B[1m{}\x1B[0m\x1B[K\r\n", line);
        } else if row == th / 2 - 1 {
            let msg = if won { "  YOU WIN!  " } else { "  YOU LOSE  " };
            let color = if won { "\x1B[1;7;32m" } else { "\x1B[1;7;31m" };
            let pad = tw.saturating_sub(msg.len()) / 2;
            print!("{}{}{}\x1B[0m\x1B[K\r\n", " ".repeat(pad), color, msg);
        } else if row == th / 2 + 1 {
            let score = format!("Final Score: {}-{}", ctx.our_wins, ctx.opp_wins);
            let pad = tw.saturating_sub(score.len()) / 2;
            print!("{}\x1B[1m{}\x1B[0m\x1B[K\r\n", " ".repeat(pad), score);
        } else if row == th / 2 + 3 {
            let matchup = format!("{} vs {}", ctx.user_id, ctx.opponent_id);
            let pad = tw.saturating_sub(matchup.len()) / 2;
            print!("{}\x1B[90m{}\x1B[0m\x1B[K\r\n", " ".repeat(pad), matchup);
        } else if row == th / 2 + 5 {
            let line = "═".repeat(tw.saturating_sub(2));
            print!(" \x1B[1m{}\x1B[0m\x1B[K\r\n", line);
        } else if row == th / 2 + 7 {
            let msg = "Press any key to exit";
            let pad = tw.saturating_sub(msg.len()) / 2;
            print!("{}\x1B[90m{}\x1B[0m\x1B[K\r\n", " ".repeat(pad), msg);
        } else {
            print!("\x1B[K\r\n");
        }
    }

    stdout().flush()?;
    Ok(())
}
