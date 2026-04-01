use colored::*;
use std::collections::HashMap;
use std::io::{stdout, Write};

use crate::api::{api_get, api_post, get_api_url, get_github_token};
use crate::models::{PcStorage, PokemonData, POKEMON_DATA};

pub fn trade_tui() {
    println!();
    println!("{}", "========================================".cyan().bold());
    println!("{}", "       POKEMON TRADE BULLETIN BOARD     ".cyan().bold());
    println!("{}", "========================================".cyan().bold());
    println!();

    // Step 1: Get GitHub token
    println!("{}", "[1/3] Authenticating with GitHub...".dimmed());
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

    // Step 2: Connect to trade server
    println!("{}", "[2/3] Connecting to trade server...".dimmed());
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
    println!("{}", "[3/3] Verifying trainer identity...".dimmed());
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

    println!();

    // Load pokemon data for enriching trade posts/offers
    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();

    loop {
        println!("{}", "Trade Bulletin Board".cyan().bold());
        println!("{}", "════════════════════".cyan());
        println!("  Logged in as: {}", user_id.cyan().bold());
        println!();
        println!("  [1] Browse open trades");
        println!("  [2] Post a trade  {}", "(one active listing at a time)".dimmed());
        println!("  [3] My trade & offers");
        println!("  [q] Quit");
        println!();
        print!("> ");
        stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim() {
            "1" => trade_browse(&token),
            "2" => trade_post(&token, &pokemon_db),
            "3" => trade_my(&token),
            "q" | "Q" => break,
            _ => println!("{}", "Invalid option.".red()),
        }
        println!();
    }
}

fn trade_browse(token: &str) {
    let result = api_get("/api/trades", token);
    let data: serde_json::Value = match result.and_then(|s| serde_json::from_str(&s).ok()) {
        Some(d) => d,
        None => { eprintln!("{}", "Could not fetch trades.".red()); return; }
    };

    let trades = data["trades"].as_array();
    if trades.map(|t| t.is_empty()).unwrap_or(true) {
        println!();
        println!("{}", "  No open trades right now.".yellow());
        return;
    }

    println!();
    println!("{}", "Open trades:".cyan().bold());
    for (i, t) in trades.unwrap().iter().enumerate() {
        let name = t["offering"]["name"].as_str().unwrap_or("???");
        let looking = t["looking_for"].as_str().unwrap_or("???");
        let poster = t["poster_id"].as_str().unwrap_or("???");
        let id = t["id"].as_str().unwrap_or("");
        let shiny = if t["offering"]["shiny"].as_bool().unwrap_or(false) { " [SHINY]" } else { "" };
        println!();
        println!("  [{}] {} offers {}{}", i + 1, poster.cyan(), name.green().bold(), shiny.yellow());
        println!("      Looking for: {}", looking.yellow());
        println!("      {}", format!("Trade ID: {}", id).dimmed());
    }

    println!();
    print!("{}", "Enter trade number to make an offer (or q to go back): ".yellow());
    stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    if input.trim() == "q" || input.trim().is_empty() { return; }

    if let Ok(idx) = input.trim().parse::<usize>() {
        if let Some(trades) = trades {
            if idx >= 1 && idx <= trades.len() {
                let trade_id = trades[idx - 1]["id"].as_str().unwrap_or("");
                trade_make_offer(token, trade_id);
            } else {
                println!("{}", "Invalid selection.".red());
            }
        }
    }
}

fn trade_make_offer(token: &str, trade_id: &str) {
    let storage = PcStorage::load();
    if storage.pokemon.is_empty() {
        eprintln!("{}", "Your PC is empty — nothing to offer!".red());
        return;
    }

    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();

    // Show PC for selection
    println!();
    println!("{}", "Your PC:".cyan());
    let mut names: Vec<String> = storage.pokemon.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names.dedup();
    for (i, name) in names.iter().enumerate() {
        let normalized = name.replace("-", "_");
        let types_str = pokemon_db.get(&normalized)
            .map(|d| d.types.join("/"))
            .unwrap_or_default();
        let count = storage.pokemon.iter().filter(|p| p.name == *name).count();
        println!("  [{}] {:15} x{}  {}", i + 1, name.green(), count, types_str.cyan());
    }

    println!();
    print!("{}", "Pokemon to offer (number or q): ".yellow());
    stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    if input.trim() == "q" || input.trim().is_empty() { return; }

    let name = match input.trim().parse::<usize>() {
        Ok(i) if i >= 1 && i <= names.len() => names[i - 1].clone(),
        _ => {
            let n = input.trim().to_lowercase();
            if storage.pokemon.iter().any(|p| p.name.to_lowercase() == n) {
                n
            } else {
                eprintln!("{}", "Invalid selection.".red());
                return;
            }
        }
    };

    let normalized = name.replace("-", "_");
    let (types, power) = pokemon_db.get(&normalized)
        .map(|d| (d.types.clone(), d.power_rank))
        .unwrap_or((vec![], 0));
    let shiny = storage.pokemon.iter()
        .find(|p| p.name.to_lowercase() == name.to_lowercase())
        .map(|p| p.shiny)
        .unwrap_or(false);

    let body = serde_json::json!({
        "trade_id": trade_id,
        "pokemon": { "name": name, "types": types, "power_rank": power, "shiny": shiny }
    }).to_string();

    match api_post("/api/trade/offer", token, &body) {
        Some(s) => {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                let msg = data["message"].as_str().unwrap_or("Offer submitted!");
                println!("{}", msg.green().bold());
            }
        }
        None => eprintln!("{}", "Failed to submit offer.".red()),
    }
}

fn trade_post(token: &str, pokemon_db: &HashMap<String, PokemonData>) {
    // Check if user already has an active listing
    if let Some(s) = api_get("/api/trade/mine", token) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
            if data["status"].as_str() != Some("none") {
                let name = data["trade"]["offering"]["name"].as_str().unwrap_or("???");
                println!();
                println!("{}", "  You already have an active trade listing:".yellow());
                println!("    Offering: {}", name.green().bold());
                println!("  Cancel it first from 'My trade & offers' before posting a new one.");
                return;
            }
        }
    }

    let storage = PcStorage::load();
    if storage.pokemon.is_empty() {
        eprintln!("{}", "Your PC is empty!".red());
        return;
    }

    println!();
    println!("{}", "Select a Pokemon to put up for trade:".cyan());
    let mut names: Vec<String> = storage.pokemon.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names.dedup();
    for (i, name) in names.iter().enumerate() {
        let normalized = name.replace("-", "_");
        let types_str = pokemon_db.get(&normalized)
            .map(|d| d.types.join("/"))
            .unwrap_or_default();
        let count = storage.pokemon.iter().filter(|p| p.name == *name).count();
        println!("  [{}] {:15} x{}  {}", i + 1, name.green(), count, types_str.cyan());
    }

    println!();
    print!("{}", "Pokemon to offer (number or q): ".yellow());
    stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    if input.trim() == "q" || input.trim().is_empty() { return; }

    let idx: usize = match input.trim().parse::<usize>() {
        Ok(i) if i >= 1 && i <= names.len() => i - 1,
        _ => { eprintln!("{}", "Invalid selection.".red()); return; }
    };
    let pokemon_name = &names[idx];

    let normalized = pokemon_name.replace("-", "_");
    let (types, power) = pokemon_db.get(&normalized)
        .map(|d| (d.types.clone(), d.power_rank))
        .unwrap_or((vec![], 0));
    let shiny = storage.pokemon.iter()
        .find(|p| p.name == *pokemon_name)
        .map(|p| p.shiny)
        .unwrap_or(false);

    println!();
    print!("{}", "What are you looking for? (e.g. \"any legendary\", \"charizard\"): ".yellow());
    stdout().flush().unwrap();
    let mut looking_for = String::new();
    std::io::stdin().read_line(&mut looking_for).unwrap();
    let looking_for = looking_for.trim();
    if looking_for.is_empty() {
        eprintln!("{}", "You must specify what you're looking for.".red());
        return;
    }

    println!();
    println!("  Posting: {} for \"{}\"", pokemon_name.green().bold(), looking_for.yellow());

    let body = serde_json::json!({
        "offering": { "name": pokemon_name, "types": types, "power_rank": power, "shiny": shiny },
        "looking_for": looking_for
    }).to_string();

    match api_post("/api/trade/create", token, &body) {
        Some(s) => {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(err) = data["error"].as_str() {
                    eprintln!("  {}", err.red());
                } else {
                    let msg = data["message"].as_str().unwrap_or("Trade posted!");
                    println!("  {}", msg.green().bold());
                }
            }
        }
        None => eprintln!("{}", "Failed to post trade.".red()),
    }
}

fn trade_my(token: &str) {
    let result = api_get("/api/trade/mine", token);
    let data: serde_json::Value = match result.and_then(|s| serde_json::from_str(&s).ok()) {
        Some(d) => d,
        None => { eprintln!("{}", "Could not fetch your trades.".red()); return; }
    };

    if data["status"].as_str() == Some("none") {
        println!();
        println!("{}", "  No active trade listing.".yellow());
        println!("  Use option [2] to post a trade.");
        return;
    }

    let trade = &data["trade"];
    let name = trade["offering"]["name"].as_str().unwrap_or("???");
    let looking = trade["looking_for"].as_str().unwrap_or("???");
    let trade_id = trade["id"].as_str().unwrap_or("");

    println!();
    println!("{}", "Your active trade:".cyan().bold());
    println!("  Offering:    {}", name.green().bold());
    println!("  Looking for: {}", looking.yellow());
    println!("  {}", format!("Trade ID: {}", trade_id).dimmed());

    let offers = data["offers"].as_array();
    let pending: Vec<&serde_json::Value> = offers.map(|o| o.iter().filter(|x| x["status"].as_str() == Some("pending")).collect()).unwrap_or_default();

    if pending.is_empty() {
        println!();
        println!("  {}", "No offers yet.".dimmed());
    } else {
        println!();
        println!("  {} pending offer(s):", pending.len().to_string().cyan());
        for (i, o) in pending.iter().enumerate() {
            let oname = o["pokemon"]["name"].as_str().unwrap_or("???");
            let from = o["offer_by_id"].as_str().unwrap_or("???");
            println!("    [{}] {} from {}", i + 1, oname.green(), from.cyan());
        }
    }

    println!();
    println!("  [a] Accept an offer");
    println!("  [r] Reject an offer");
    println!("  [c] Cancel this trade");
    println!("  [q] Back");
    println!();
    print!("> ");
    stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    match input.trim() {
        "a" | "A" => {
            if pending.is_empty() {
                println!("{}", "  No pending offers to accept.".yellow());
                return;
            }
            print!("{}", "  Accept offer number: ".yellow());
            stdout().flush().unwrap();
            let mut num = String::new();
            std::io::stdin().read_line(&mut num).unwrap();
            if let Ok(idx) = num.trim().parse::<usize>() {
                if idx >= 1 && idx <= pending.len() {
                    let offer_id = pending[idx - 1]["id"].as_str().unwrap_or("");
                    let body = serde_json::json!({
                        "trade_id": trade_id,
                        "offer_id": offer_id
                    }).to_string();
                    match api_post("/api/trade/accept", token, &body) {
                        Some(s) => {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                                let msg = data["message"].as_str().unwrap_or("Trade accepted!");
                                println!("  {}", msg.green().bold());
                            }
                        }
                        None => eprintln!("{}", "  Failed to accept offer.".red()),
                    }
                } else {
                    println!("{}", "  Invalid selection.".red());
                }
            }
        }
        "r" | "R" => {
            if pending.is_empty() {
                println!("{}", "  No pending offers to reject.".yellow());
                return;
            }
            print!("{}", "  Reject offer number: ".yellow());
            stdout().flush().unwrap();
            let mut num = String::new();
            std::io::stdin().read_line(&mut num).unwrap();
            if let Ok(idx) = num.trim().parse::<usize>() {
                if idx >= 1 && idx <= pending.len() {
                    let offer_id = pending[idx - 1]["id"].as_str().unwrap_or("");
                    let body = serde_json::json!({
                        "trade_id": trade_id,
                        "offer_id": offer_id
                    }).to_string();
                    match api_post("/api/trade/reject", token, &body) {
                        Some(s) => {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                                let msg = data["message"].as_str().unwrap_or("Offer rejected.");
                                println!("  {}", msg.green().bold());
                            }
                        }
                        None => eprintln!("{}", "  Failed to reject offer.".red()),
                    }
                } else {
                    println!("{}", "  Invalid selection.".red());
                }
            }
        }
        "c" | "C" => {
            print!("{}", "  Are you sure? This will cancel your trade and reject all offers. (y/n): ".red());
            stdout().flush().unwrap();
            let mut confirm = String::new();
            std::io::stdin().read_line(&mut confirm).unwrap();
            if confirm.trim().to_lowercase() == "y" {
                let body = serde_json::json!({
                    "trade_id": trade_id
                }).to_string();
                match api_post("/api/trade/cancel", token, &body) {
                    Some(s) => {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                            let msg = data["message"].as_str().unwrap_or("Trade cancelled.");
                            println!("  {}", msg.green().bold());
                        }
                    }
                    None => eprintln!("{}", "  Failed to cancel trade.".red()),
                }
            } else {
                println!("  Cancelled.");
            }
        }
        _ => {}
    }
}
