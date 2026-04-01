use colored::*;
use std::collections::HashMap;
use std::io::{stdout, Write};
use std::thread;
use std::time::Duration;

use crate::api::{api_get, api_post, get_api_url, get_github_token};
use crate::models::{BattleTeam, PcStorage, PokemonData, POKEMON_DATA};

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

    // Step 2: Connect to battle server and authenticate
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

    // Step 3: Authenticate user with the server
    println!("{}", "[3/5] Verifying trainer identity...".dimmed());
    let me_result = api_get("/api/me", &token);
    let user_id = match me_result {
        Some(s) => {
            match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(data) => {
                    let uid = data["user_id"].as_str().unwrap_or("unknown").to_string();
                    println!("  {} Authenticated as {}", "OK".green().bold(), uid.cyan().bold());
                    uid
                }
                Err(_) => {
                    eprintln!("  {} Server returned invalid response", "FAIL".red().bold());
                    return;
                }
            }
        }
        None => {
            eprintln!("  {} Authentication failed — check your GitHub token", "FAIL".red().bold());
            eprintln!("  Run: {}", "gh auth login".yellow());
            return;
        }
    };

    // Step 4: Load and validate PC + battle team
    println!("{}", "[4/5] Loading battle data...".dimmed());
    let storage = PcStorage::load();
    let total_pokemon = storage.pokemon.len();
    if total_pokemon < 6 {
        eprintln!("  {} You have {} Pokemon — need at least 6", "FAIL".red().bold(), total_pokemon);
        eprintln!("  Go catch more Pokemon first!");
        return;
    }
    println!("  {} PC loaded ({} Pokemon)", "OK".green().bold(), total_pokemon);

    let team = BattleTeam::load();
    if team.pokemon.is_empty() {
        eprintln!("  {} Battle team is empty", "FAIL".red().bold());
        eprintln!("  Add Pokemon with: {}", "catch-pokemon team --add <name>".yellow());
        return;
    }
    println!("  {} Battle team loaded ({} Pokemon)", "OK".green().bold(), team.pokemon.len());

    // Build PC data for API
    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();
    let pc_pokemon: Vec<serde_json::Value> = storage.pokemon.iter().map(|p| {
        let normalized = p.name.replace("-", "_");
        let (types, power) = pokemon_db.get(&normalized)
            .map(|d| (d.types.clone(), d.power_rank))
            .unwrap_or((vec![], 0));
        serde_json::json!({
            "name": p.name,
            "types": types,
            "power_rank": power,
            "shiny": p.shiny
        })
    }).collect();

    if pc_pokemon.len() < 6 {
        eprintln!("  {} Not enough valid Pokemon in PC", "FAIL".red().bold());
        return;
    }

    // Step 5: Join matchmaking queue
    println!("{}", "[5/5] Searching for opponent...".dimmed());
    println!("  Waiting up to 60 seconds for a match...");
    println!();

    let body = serde_json::json!({ "pc": pc_pokemon }).to_string();
    let match_result = api_post("/api/battle/join", &token, &body);

    let match_data: serde_json::Value = match match_result {
        Some(s) => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(_) => {
                eprintln!("{}", "  Invalid response from server.".red());
                return;
            }
        },
        None => {
            eprintln!("{}", "  No opponent found or connection lost.".red());
            return;
        }
    };

    let status = match_data["status"].as_str().unwrap_or("");
    if status == "timeout" {
        println!("{}", "  No opponent found. Try again later.".yellow());
        return;
    }
    if status != "matched" {
        let msg = match_data["error"].as_str().or(match_data["message"].as_str()).unwrap_or("Unknown error");
        eprintln!("  {}", msg.red());
        return;
    }

    let battle_id = match_data["battle_id"].as_str().unwrap_or("");
    let opponent = match_data["opponent_id"].as_str().unwrap_or("???");

    println!("{}", "========================================".green().bold());
    println!("  {} vs {}", user_id.cyan().bold(), opponent.magenta().bold());
    println!("  Battle ID: {}", battle_id.dimmed());
    println!("{}", "========================================".green().bold());
    println!();

    // Show opponent's PC
    println!("{}", "Opponent's Pokemon:".yellow().bold());
    if let Some(opp_pc) = match_data["opponent_pc"].as_array() {
        for p in opp_pc {
            let name = p["name"].as_str().unwrap_or("???");
            let power = p["power_rank"].as_u64().unwrap_or(0);
            let types: Vec<&str> = p["types"].as_array()
                .map(|a| a.iter().filter_map(|t| t.as_str()).collect())
                .unwrap_or_default();
            let shiny = if p["shiny"].as_bool().unwrap_or(false) { " [SHINY]" } else { "" };
            println!("  {:15} Power: {:3}  Type: {}{}", name.green(), power.to_string().bright_yellow(), types.join("/").cyan(), shiny.yellow());
        }
    }

    println!();
    println!("{}", "Best of 5 rounds — pick 6 Pokemon each round".dimmed());
    println!("{}", "Formula: 40% power + 40% type advantage + 20% RNG".dimmed());
    println!();

    // Battle rounds
    let mut round = 1;
    loop {
        println!("{}", format!("--- Round {} ---", round).cyan().bold());
        println!();

        // Show battle team for selection
        println!("{}", "Your battle team:".yellow());
        let team_pokemon: Vec<serde_json::Value> = team.pokemon.iter().enumerate().map(|(i, p)| {
            let normalized = p.name.replace("-", "_");
            let (types, power) = pokemon_db.get(&normalized)
                .map(|d| (d.types.clone(), d.power_rank))
                .unwrap_or((vec![], 0));
            let shiny = if p.shiny { " [SHINY]" } else { "" };
            println!("  [{}] {:15} Power: {:3}  Type: {}{}", i + 1, p.name.green(), power.to_string().bright_yellow(), types.join("/").cyan(), shiny.yellow());
            serde_json::json!({
                "name": p.name,
                "types": types,
                "power_rank": power,
                "shiny": p.shiny
            })
        }).collect();

        if team_pokemon.len() < 6 {
            eprintln!("{}", "Not enough Pokemon in battle team. Add more with: catch-pokemon team --add <name>".red());
            return;
        }

        println!();
        print!("{}", "Select 6 by number (e.g. 1 2 3 4 5 6): ".yellow());
        stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        let picks: Vec<usize> = input.trim().split_whitespace()
            .filter_map(|s| s.parse::<usize>().ok())
            .filter(|&n| n >= 1 && n <= team_pokemon.len())
            .map(|n| n - 1)
            .collect();

        if picks.len() != 6 {
            println!("{}", "Pick exactly 6. Try again.".red());
            continue;
        }

        let selected_team: Vec<serde_json::Value> = picks.iter().map(|&i| team_pokemon[i].clone()).collect();

        // Submit team
        println!("{}", "Locking in team...".yellow());
        let select_body = serde_json::json!({
            "battle_id": battle_id,
            "team": selected_team
        }).to_string();
        api_post("/api/battle/select", &token, &select_body);
        println!("{}", "Team locked in. Waiting for opponent...".green());

        // Poll for results
        loop {
            thread::sleep(Duration::from_secs(1));
            let status_result = api_get("/api/battle/status", &token);
            if let Some(s) = status_result {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                    let bstatus = data["status"].as_str().unwrap_or("");
                    let rounds = data["rounds"].as_array().map(|a| a.len()).unwrap_or(0);

                    if rounds >= round {
                        // Show result
                        if let Some(r) = data["rounds"].as_array().and_then(|a| a.last()) {
                            let res = &r["result"];
                            let me = data["you"].as_str().unwrap_or("");
                            let winner = res["winner"].as_str().unwrap_or("");
                            let p1w = data["p1_wins"].as_u64().unwrap_or(0);
                            let p2w = data["p2_wins"].as_u64().unwrap_or(0);

                            println!();
                            println!("  Your Score:     {:.3}", res["p1_score"].as_f64().unwrap_or(0.0));
                            println!("  Opponent Score: {:.3}", res["p2_score"].as_f64().unwrap_or(0.0));

                            if winner == me {
                                println!("  {}", "You won this round!".green().bold());
                            } else {
                                println!("  {}", "You lost this round.".red().bold());
                            }
                            println!("  Series: {}-{}", p1w, p2w);
                        }
                        break;
                    }

                    if bstatus == "complete" || bstatus == "abandoned" || bstatus == "none" {
                        let msg = data["message"].as_str().unwrap_or("Battle ended.");
                        println!();
                        println!("{}", "========================================".bold());
                        println!("{}", msg.bold());
                        println!("{}", "========================================".bold());
                        return;
                    }
                }
            }
        }

        // Check if battle is over
        if let Some(s) = api_get("/api/battle/status", &token) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                let bstatus = data["status"].as_str().unwrap_or("");
                if bstatus == "complete" {
                    let msg = data["message"].as_str().unwrap_or("Battle ended.");
                    println!();
                    println!("{}", "========================================".bold());
                    println!("{}", msg.bold());
                    println!("{}", "========================================".bold());
                    return;
                }
            }
        }

        round += 1;
        println!();
    }
}
