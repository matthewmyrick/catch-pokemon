use colored::*;
use std::io::{stdout, Write};

use crate::api::{api_get, api_post, get_api_url, get_github_token};
use crate::models::PcStorage;

pub fn trade_tui() {
    let token = match get_github_token() {
        Some(t) => t,
        None => {
            eprintln!("{}", "Not logged in to GitHub. Run: gh auth login".red().bold());
            return;
        }
    };

    println!("{}", "Connecting to trade server...".cyan());
    if api_get("/health", &token).is_none() {
        eprintln!("{}", format!("Could not connect to server at {}", get_api_url()).red());
        return;
    }
    println!("{}", "Connected.".green());
    println!();

    loop {
        println!("{}", "Trade Bulletin Board".cyan().bold());
        println!("{}", "════════════════════".cyan());
        println!();
        println!("  [1] Browse open trades");
        println!("  [2] Post a trade");
        println!("  [3] My trades");
        println!("  [q] Quit");
        println!();
        print!("> ");
        stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        match input.trim() {
            "1" => trade_browse(&token),
            "2" => trade_post(&token),
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
        println!("{}", "No open trades right now.".yellow());
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
    if input.trim() == "q" { return; }

    if let Ok(idx) = input.trim().parse::<usize>() {
        if let Some(trades) = trades {
            if idx >= 1 && idx <= trades.len() {
                let trade_id = trades[idx - 1]["id"].as_str().unwrap_or("");
                trade_make_offer(token, trade_id);
            }
        }
    }
}

fn trade_make_offer(token: &str, trade_id: &str) {
    println!();
    print!("Pokemon to offer: ");
    stdout().flush().unwrap();
    let mut name = String::new();
    std::io::stdin().read_line(&mut name).unwrap();
    let name = name.trim().to_lowercase();

    // Check PC
    let storage = PcStorage::load();
    if !storage.pokemon.iter().any(|p| p.name.to_lowercase() == name) {
        eprintln!("{}", format!("You don't have {} in your PC.", name).red());
        return;
    }

    let body = serde_json::json!({
        "trade_id": trade_id,
        "pokemon": { "name": name, "types": [], "power_rank": 0, "shiny": false }
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

fn trade_post(token: &str) {
    let storage = PcStorage::load();
    if storage.pokemon.is_empty() {
        eprintln!("{}", "Your PC is empty!".red());
        return;
    }

    println!();
    println!("{}", "Your PC:".cyan());
    let mut names: Vec<String> = storage.pokemon.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names.dedup();
    for (i, name) in names.iter().enumerate() {
        println!("  [{}] {}", i + 1, name.green());
    }

    println!();
    print!("Pokemon to offer (number): ");
    stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let idx: usize = match input.trim().parse::<usize>() {
        Ok(i) if i >= 1 && i <= names.len() => i - 1,
        _ => { eprintln!("{}", "Invalid selection.".red()); return; }
    };
    let pokemon_name = &names[idx];

    print!("What are you looking for? ");
    stdout().flush().unwrap();
    let mut looking_for = String::new();
    std::io::stdin().read_line(&mut looking_for).unwrap();

    let body = serde_json::json!({
        "offering": { "name": pokemon_name, "types": [], "power_rank": 0, "shiny": false },
        "looking_for": looking_for.trim()
    }).to_string();

    match api_post("/api/trade/create", token, &body) {
        Some(s) => {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                let msg = data["message"].as_str().unwrap_or("Trade posted!");
                println!("{}", msg.green().bold());
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
        println!("{}", "No active trade listing.".yellow());
        return;
    }

    let trade = &data["trade"];
    let name = trade["offering"]["name"].as_str().unwrap_or("???");
    let looking = trade["looking_for"].as_str().unwrap_or("???");
    println!();
    println!("  Offering: {}", name.green().bold());
    println!("  Looking for: {}", looking.yellow());

    let offers = data["offers"].as_array();
    let pending: Vec<&serde_json::Value> = offers.map(|o| o.iter().filter(|x| x["status"].as_str() == Some("pending")).collect()).unwrap_or_default();

    if pending.is_empty() {
        println!("  {}", "No offers yet.".dimmed());
        return;
    }

    println!();
    println!("  {} pending offer(s):", pending.len().to_string().cyan());
    for (i, o) in pending.iter().enumerate() {
        let oname = o["pokemon"]["name"].as_str().unwrap_or("???");
        let from = o["offer_by_id"].as_str().unwrap_or("???");
        let oid = o["id"].as_str().unwrap_or("");
        println!("    [{}] {} from {}", i + 1, oname.green(), from.cyan());
        println!("        {}", format!("Offer ID: {}", oid).dimmed());
    }

    println!();
    print!("Accept an offer? (number or q): ");
    stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    if input.trim() == "q" { return; }

    if let Ok(idx) = input.trim().parse::<usize>() {
        if idx >= 1 && idx <= pending.len() {
            let offer_id = pending[idx - 1]["id"].as_str().unwrap_or("");
            let trade_id = trade["id"].as_str().unwrap_or("");
            let body = serde_json::json!({
                "trade_id": trade_id,
                "offer_id": offer_id
            }).to_string();
            match api_post("/api/trade/accept", token, &body) {
                Some(s) => {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                        let msg = data["message"].as_str().unwrap_or("Trade accepted!");
                        println!("{}", msg.green().bold());
                    }
                }
                None => eprintln!("{}", "Failed to accept offer.".red()),
            }
        }
    }
}
