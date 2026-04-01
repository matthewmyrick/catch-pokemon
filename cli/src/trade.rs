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
use crate::models::{PcStorage, PokemonData, POKEMON_DATA};

// --- Data types ---

struct TradeListing {
    id: String,
    poster: String,
    offering_name: String,
    offering_types: Vec<String>,
    offering_power: u64,
    offering_shiny: bool,
    looking_for: String,
}

struct TradeOfferEntry {
    id: String,
    from: String,
    pokemon_name: String,
    pokemon_types: Vec<String>,
    pokemon_power: u64,
    pokemon_shiny: bool,
}

struct PcPokemonEntry {
    name: String,
    types: Vec<String>,
    power: u8,
    count: usize,
    shiny: bool,
}

#[derive(PartialEq)]
enum TradeTab { Browse, Post, MyTrade }

#[derive(PartialEq, Clone)]
enum TradeMode { Normal, Offering(String), InputLookingFor, ConfirmPost }

struct TradeContext {
    token: String,
    user_id: String,
    active_tab: TradeTab,
    mode: TradeMode,

    // Browse
    trades: Vec<TradeListing>,
    browse_selected: usize,
    browse_scroll: usize,

    // Post
    pc_pokemon: Vec<PcPokemonEntry>,
    post_selected: usize,
    post_scroll: usize,
    looking_for_input: String,
    has_active_trade: bool,

    // My Trade
    my_trade: Option<TradeListing>,
    my_offers: Vec<TradeOfferEntry>,
    offer_selected: usize,
    offer_scroll: usize,

    search_term: String,
    searching: bool,
    status_msg: Option<String>,
    confirming: Option<String>, // "accept", "reject", "cancel"
}

// --- Auth + entry ---

pub fn trade_tui() {
    println!();
    println!("{}", "========================================".cyan().bold());
    println!("{}", "       POKEMON TRADE BULLETIN BOARD     ".cyan().bold());
    println!("{}", "========================================".cyan().bold());
    println!();

    println!("{}", "[1/3] Authenticating with GitHub...".dimmed());
    let token = match get_github_token() {
        Some(t) => { println!("  {} GitHub token found", "OK".green().bold()); t }
        None => { eprintln!("  {} Not logged in. Run: gh auth login", "FAIL".red().bold()); return; }
    };

    println!("{}", "[2/3] Connecting to trade server...".dimmed());
    let server_url = get_api_url();
    if api_get("/health", &token).is_none() {
        eprintln!("  {} Could not connect to {}", "FAIL".red().bold(), server_url);
        return;
    }
    println!("  {} Server at {}", "OK".green().bold(), server_url.cyan());

    println!("{}", "[3/3] Verifying trainer identity...".dimmed());
    let user_id = match api_get("/api/me", &token) {
        Some(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(data) => {
                let uid = data["user_id"].as_str().unwrap_or("unknown").to_string();
                println!("  {} Authenticated as {}", "OK".green().bold(), uid.cyan().bold());
                uid
            }
            Err(_) => { eprintln!("  {} Invalid response", "FAIL".red().bold()); return; }
        },
        None => { eprintln!("  {} Authentication failed", "FAIL".red().bold()); return; }
    };

    // Load PC
    let pokemon_db: HashMap<String, PokemonData> = serde_json::from_str(POKEMON_DATA).unwrap_or_default();
    let storage = PcStorage::load();
    let mut pc_pokemon = build_pc_entries(&storage, &pokemon_db);
    pc_pokemon.sort_by(|a, b| a.name.cmp(&b.name));

    let mut ctx = TradeContext {
        token: token.clone(),
        user_id,
        active_tab: TradeTab::Browse,
        mode: TradeMode::Normal,
        trades: Vec::new(),
        browse_selected: 0, browse_scroll: 0,
        pc_pokemon,
        post_selected: 0, post_scroll: 0,
        looking_for_input: String::new(),
        has_active_trade: false,
        my_trade: None, my_offers: Vec::new(),
        offer_selected: 0, offer_scroll: 0,
        search_term: String::new(), searching: false,
        status_msg: None, confirming: None,
    };

    // Initial data fetch
    refresh_trades(&mut ctx);
    refresh_my_trade(&mut ctx);

    if let Err(e) = run_trade_tui(&mut ctx) {
        eprintln!("Trade TUI error: {}", e);
    }
}

fn build_pc_entries(storage: &PcStorage, pokemon_db: &HashMap<String, PokemonData>) -> Vec<PcPokemonEntry> {
    let mut map: HashMap<String, PcPokemonEntry> = HashMap::new();
    for p in &storage.pokemon {
        let normalized = p.name.replace("-", "_");
        let (types, power) = pokemon_db.get(&normalized)
            .map(|d| (d.types.clone(), d.power_rank))
            .unwrap_or((vec![], 0));
        let entry = map.entry(p.name.clone()).or_insert(PcPokemonEntry {
            name: p.name.clone(), types, power, count: 0, shiny: p.shiny,
        });
        entry.count += 1;
        if p.shiny { entry.shiny = true; }
    }
    map.into_values().collect()
}

fn refresh_trades(ctx: &mut TradeContext) {
    ctx.trades.clear();
    if let Some(s) = api_get("/api/trades", &ctx.token) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(arr) = data["trades"].as_array() {
                for t in arr {
                    ctx.trades.push(TradeListing {
                        id: t["id"].as_str().unwrap_or("").to_string(),
                        poster: t["poster_id"].as_str().unwrap_or("???").to_string(),
                        offering_name: t["offering"]["name"].as_str().unwrap_or("???").to_string(),
                        offering_types: t["offering"]["types"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                        offering_power: t["offering"]["power_rank"].as_u64().unwrap_or(0),
                        offering_shiny: t["offering"]["shiny"].as_bool().unwrap_or(false),
                        looking_for: t["looking_for"].as_str().unwrap_or("???").to_string(),
                    });
                }
            }
        }
    }
    if ctx.browse_selected >= ctx.trades.len() && !ctx.trades.is_empty() {
        ctx.browse_selected = ctx.trades.len() - 1;
    }
}

fn refresh_my_trade(ctx: &mut TradeContext) {
    ctx.my_trade = None;
    ctx.my_offers.clear();
    ctx.has_active_trade = false;
    if let Some(s) = api_get("/api/trade/mine", &ctx.token) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
            if data["status"].as_str() != Some("none") {
                let t = &data["trade"];
                ctx.my_trade = Some(TradeListing {
                    id: t["id"].as_str().unwrap_or("").to_string(),
                    poster: ctx.user_id.clone(),
                    offering_name: t["offering"]["name"].as_str().unwrap_or("???").to_string(),
                    offering_types: t["offering"]["types"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                    offering_power: t["offering"]["power_rank"].as_u64().unwrap_or(0),
                    offering_shiny: t["offering"]["shiny"].as_bool().unwrap_or(false),
                    looking_for: t["looking_for"].as_str().unwrap_or("???").to_string(),
                });
                ctx.has_active_trade = true;

                if let Some(arr) = data["offers"].as_array() {
                    for o in arr {
                        if o["status"].as_str() == Some("pending") {
                            ctx.my_offers.push(TradeOfferEntry {
                                id: o["id"].as_str().unwrap_or("").to_string(),
                                from: o["offer_by_id"].as_str().unwrap_or("???").to_string(),
                                pokemon_name: o["pokemon"]["name"].as_str().unwrap_or("???").to_string(),
                                pokemon_types: o["pokemon"]["types"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                                pokemon_power: o["pokemon"]["power_rank"].as_u64().unwrap_or(0),
                                pokemon_shiny: o["pokemon"]["shiny"].as_bool().unwrap_or(false),
                            });
                        }
                    }
                }
            }
        }
    }
}

// --- Main TUI loop ---

fn run_trade_tui(ctx: &mut TradeContext) -> Result<(), Box<dyn std::error::Error>> {
    stdout().execute(crossterm::terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    stdout().execute(cursor::Hide)?;

    let result = trade_loop(ctx);

    stdout().execute(cursor::Show)?;
    terminal::disable_raw_mode()?;
    stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;
    result
}

fn trade_loop(ctx: &mut TradeContext) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match ctx.active_tab {
            TradeTab::Browse => render_browse(ctx)?,
            TradeTab::Post => render_post(ctx)?,
            TradeTab::MyTrade => render_my_trade(ctx)?,
        }

        // Drain events
        while event::poll(Duration::from_millis(0))? { let _ = event::read(); }

        if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
            // Global keys
            if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) { break; }

            if ctx.searching {
                match code {
                    KeyCode::Esc | KeyCode::Enter => { ctx.searching = false; }
                    KeyCode::Backspace => { ctx.search_term.pop(); }
                    KeyCode::Char(c) => { ctx.search_term.push(c); }
                    _ => {}
                }
                continue;
            }

            if let Some(ref action) = ctx.confirming.clone() {
                if code == KeyCode::Char('y') || code == KeyCode::Char('Y') {
                    do_confirm_action(ctx, action);
                } else {
                    ctx.status_msg = Some("Cancelled.".to_string());
                }
                ctx.confirming = None;
                continue;
            }

            // Tab switching (only in Normal mode)
            if ctx.mode == TradeMode::Normal {
                match code {
                    KeyCode::Char('1') => {
                        ctx.active_tab = TradeTab::Browse;
                        ctx.search_term.clear();
                        refresh_trades(ctx);
                        continue;
                    }
                    KeyCode::Char('2') => {
                        ctx.active_tab = TradeTab::Post;
                        ctx.search_term.clear();
                        refresh_my_trade(ctx);
                        continue;
                    }
                    KeyCode::Char('3') => {
                        ctx.active_tab = TradeTab::MyTrade;
                        ctx.search_term.clear();
                        refresh_my_trade(ctx);
                        continue;
                    }
                    _ => {}
                }
            }

            // Tab-specific input
            let should_quit = match ctx.active_tab {
                TradeTab::Browse => handle_browse_input(ctx, code)?,
                TradeTab::Post => handle_post_input(ctx, code)?,
                TradeTab::MyTrade => handle_my_trade_input(ctx, code)?,
            };
            if should_quit { break; }
        }
    }
    Ok(())
}

// --- Tab header ---

fn render_tab_header(ctx: &TradeContext, _tw: usize) {
    let t1 = if ctx.active_tab == TradeTab::Browse { "\x1B[1;7;36m 1 Browse \x1B[0m" } else { "\x1B[90m 1 Browse \x1B[0m" };
    let t2 = if ctx.active_tab == TradeTab::Post { "\x1B[1;7;36m 2 Post \x1B[0m" } else { "\x1B[90m 2 Post \x1B[0m" };
    let t3 = if ctx.active_tab == TradeTab::MyTrade { "\x1B[1;7;36m 3 My Trade \x1B[0m" } else { "\x1B[90m 3 My Trade \x1B[0m" };
    print!(" \x1B[1;36mTrade Board\x1B[0m  {} {} {}  \x1B[90m{}\x1B[0m\x1B[K\r\n", t1, t2, t3, ctx.user_id);
}

// --- Browse Tab ---

fn render_browse(ctx: &TradeContext) -> Result<(), Box<dyn std::error::Error>> {
    let (tw, th) = terminal::size()?;
    let tw = tw as usize;
    let th = th as usize;
    let left_width = 40.min(tw / 2);
    let list_height = th.saturating_sub(6);

    let filtered: Vec<usize> = ctx.trades.iter().enumerate()
        .filter(|(_, t)| {
            if ctx.search_term.is_empty() { return true; }
            let s = ctx.search_term.to_lowercase();
            t.offering_name.to_lowercase().contains(&s) || t.poster.to_lowercase().contains(&s) || t.looking_for.to_lowercase().contains(&s)
        })
        .map(|(i, _)| i)
        .collect();

    stdout().execute(cursor::MoveTo(0, 0))?;
    render_tab_header(ctx, tw);

    // Search / sub-header
    if ctx.searching {
        print!(" {}: {}{}\x1B[K\r\n", "Search".cyan().bold(), ctx.search_term.yellow(), "\x1B[33m|\x1B[0m");
    } else if let TradeMode::Offering(ref trade_id) = ctx.mode {
        print!(" \x1B[1;33mSelect a Pokemon to offer for trade {}\x1B[0m\x1B[K\r\n", &trade_id[..8.min(trade_id.len())]);
    } else if !ctx.search_term.is_empty() {
        print!(" Search: {} ({} results)\x1B[K\r\n", ctx.search_term.yellow(), filtered.len());
    } else {
        print!(" \x1B[90m{} open trade(s)\x1B[0m\x1B[K\r\n", filtered.len());
    }

    print!(" {}\x1B[K\r\n", "\x1B[90m-\x1B[0m".repeat(tw.saturating_sub(2)));

    if let TradeMode::Offering(_) = ctx.mode {
        // Show PC for offering
        for row in 0..list_height {
            let idx = ctx.post_scroll + row;
            let left_cell = if idx < ctx.pc_pokemon.len() {
                let p = &ctx.pc_pokemon[idx];
                let _types_str = p.types.iter().map(|t| color_type(t)).collect::<Vec<_>>().join("/");
                if idx == ctx.post_selected {
                    format!(" \x1B[7m > {:<15} x{} P:{:<3}\x1B[0m", p.name, p.count, p.power)
                } else {
                    format!("   \x1B[32m{:<15}\x1B[0m x{} \x1B[33mP:{:<3}\x1B[0m", p.name, p.count, p.power)
                }
            } else { String::new() };

            let right_cell = if row == 0 {
                " \x1B[1;36mYour PC\x1B[0m".to_string()
            } else if row == 2 && ctx.post_selected < ctx.pc_pokemon.len() {
                let p = &ctx.pc_pokemon[ctx.post_selected];
                let types_str = p.types.iter().map(|t| color_type(t)).collect::<Vec<_>>().join(" / ");
                format!(" {}", types_str)
            } else if row == 3 && ctx.post_selected < ctx.pc_pokemon.len() {
                let p = &ctx.pc_pokemon[ctx.post_selected];
                format!(" Power: \x1B[33m{}\x1B[0m", p.power)
            } else { String::new() };

            print!("{}\x1B[{}G\x1B[90m|\x1B[0m{}\x1B[K\r\n", left_cell, left_width + 1, right_cell);
        }
    } else {
        // Show trades list
        for row in 0..list_height {
            let idx = ctx.browse_scroll + row;
            let fi = if idx < filtered.len() { Some(filtered[idx]) } else { None };

            let left_cell = if let Some(ti) = fi {
                let t = &ctx.trades[ti];
                let shiny = if t.offering_shiny { "*" } else { " " };
                let name_w = left_width.saturating_sub(16);
                let display = format!("{}{}", t.offering_name, shiny);
                let truncated: String = display.chars().take(name_w).collect();
                let padded = format!("{:<w$}", truncated, w = name_w);
                let poster: String = t.poster.chars().take(12).collect();

                if idx == ctx.browse_selected {
                    format!(" \x1B[7m> {:<w$} {:<12}\x1B[0m", padded, poster, w = name_w)
                } else {
                    format!("   \x1B[32m{:<w$}\x1B[0m \x1B[90m{:<12}\x1B[0m", padded, poster, w = name_w)
                }
            } else { format!("{:<w$}", "", w = left_width) };

            // Right panel — details for selected trade
            let sel_idx = if ctx.browse_selected < filtered.len() { Some(filtered[ctx.browse_selected]) } else { None };
            let right_cell = if let Some(si) = sel_idx {
                let t = &ctx.trades[si];
                match row {
                    0 => format!(" \x1B[1;36m{}\x1B[0m{}", t.offering_name, if t.offering_shiny { " \x1B[1;33m[SHINY]\x1B[0m" } else { "" }),
                    1 => String::new(),
                    2 => { let types_str = t.offering_types.iter().map(|tp| color_type(tp)).collect::<Vec<_>>().join(" / "); format!(" Type:    {}", types_str) },
                    3 => format!(" Power:   \x1B[33m{}\x1B[0m", t.offering_power),
                    4 => format!(" Poster:  \x1B[32m{}\x1B[0m", t.poster),
                    5 => String::new(),
                    6 => format!(" \x1B[1mLooking for:\x1B[0m"),
                    7 => format!("   \x1B[33m{}\x1B[0m", t.looking_for),
                    9 => format!(" \x1B[90mTrade ID: {}\x1B[0m", t.id),
                    _ => String::new(),
                }
            } else { String::new() };

            print!("{}\x1B[{}G\x1B[90m|\x1B[0m{}\x1B[K\r\n", left_cell, left_width + 1, right_cell);
        }
    }

    // Footer
    print!(" {}\x1B[K\r\n", "\x1B[90m-\x1B[0m".repeat(tw.saturating_sub(2)));
    if let Some(ref msg) = ctx.status_msg {
        print!(" \x1B[1;33m{}\x1B[0m\x1B[K", msg);
    } else if let TradeMode::Offering(_) = ctx.mode {
        print!(" \x1B[90m↑↓: Nav | Enter: Offer this Pokemon | Esc: Cancel\x1B[0m\x1B[K");
    } else if ctx.trades.is_empty() {
        print!(" \x1B[90mNo trades yet. Press 2 to post one! | q: Quit\x1B[0m\x1B[K");
    } else {
        print!(" \x1B[90m↑↓: Nav | /: Search | Enter: Make offer | r: Refresh | q: Quit\x1B[0m\x1B[K");
    }
    stdout().flush()?;
    Ok(())
}

fn handle_browse_input(ctx: &mut TradeContext, code: KeyCode) -> Result<bool, Box<dyn std::error::Error>> {
    let (_, th) = terminal::size()?;
    let list_height = (th as usize).saturating_sub(6);
    ctx.status_msg = None;

    if let TradeMode::Offering(ref trade_id) = ctx.mode.clone() {
        match code {
            KeyCode::Esc => { ctx.mode = TradeMode::Normal; }
            KeyCode::Up | KeyCode::Char('k') => { if ctx.post_selected > 0 { ctx.post_selected -= 1; } if ctx.post_selected < ctx.post_scroll { ctx.post_scroll = ctx.post_selected; } }
            KeyCode::Down | KeyCode::Char('j') => {
                if ctx.post_selected + 1 < ctx.pc_pokemon.len() { ctx.post_selected += 1; }
                if ctx.post_selected >= ctx.post_scroll + list_height { ctx.post_scroll = ctx.post_selected + 1 - list_height; }
            }
            KeyCode::Enter => {
                if ctx.post_selected < ctx.pc_pokemon.len() {
                    let p = &ctx.pc_pokemon[ctx.post_selected];
                    let body = serde_json::json!({
                        "trade_id": trade_id,
                        "pokemon": { "name": p.name, "types": p.types, "power_rank": p.power, "shiny": p.shiny }
                    }).to_string();
                    match api_post("/api/trade/offer", &ctx.token, &body) {
                        Some(s) => {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                                ctx.status_msg = Some(data["message"].as_str().unwrap_or("Offer submitted!").to_string());
                            }
                        }
                        None => { ctx.status_msg = Some("Failed to submit offer.".to_string()); }
                    }
                    ctx.mode = TradeMode::Normal;
                    refresh_trades(ctx);
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(true),
        KeyCode::Char('/') => { ctx.searching = true; ctx.search_term.clear(); }
        KeyCode::Char('r') | KeyCode::Char('R') => { refresh_trades(ctx); ctx.status_msg = Some("Refreshed.".to_string()); }
        KeyCode::Up | KeyCode::Char('k') => { if ctx.browse_selected > 0 { ctx.browse_selected -= 1; } if ctx.browse_selected < ctx.browse_scroll { ctx.browse_scroll = ctx.browse_selected; } }
        KeyCode::Down | KeyCode::Char('j') => {
            if ctx.browse_selected + 1 < ctx.trades.len() { ctx.browse_selected += 1; }
            if ctx.browse_selected >= ctx.browse_scroll + list_height { ctx.browse_scroll = ctx.browse_selected + 1 - list_height; }
        }
        KeyCode::Enter => {
            if ctx.browse_selected < ctx.trades.len() {
                let trade = &ctx.trades[ctx.browse_selected];
                if trade.poster == ctx.user_id {
                    ctx.status_msg = Some("Can't offer on your own trade.".to_string());
                } else {
                    ctx.mode = TradeMode::Offering(trade.id.clone());
                    ctx.post_selected = 0;
                    ctx.post_scroll = 0;
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

// --- Post Tab ---

fn render_post(ctx: &TradeContext) -> Result<(), Box<dyn std::error::Error>> {
    let (tw, th) = terminal::size()?;
    let tw = tw as usize;
    let th = th as usize;
    let left_width = 40.min(tw / 2);
    let list_height = th.saturating_sub(6);

    let filtered: Vec<usize> = ctx.pc_pokemon.iter().enumerate()
        .filter(|(_, p)| {
            if ctx.search_term.is_empty() { return true; }
            p.name.to_lowercase().contains(&ctx.search_term.to_lowercase())
        })
        .map(|(i, _)| i)
        .collect();

    stdout().execute(cursor::MoveTo(0, 0))?;
    render_tab_header(ctx, tw);

    if ctx.has_active_trade {
        print!(" \x1B[1;33mYou already have an active listing. Go to My Trade to manage it.\x1B[0m\x1B[K\r\n");
    } else if ctx.searching {
        print!(" {}: {}{}\x1B[K\r\n", "Search".cyan().bold(), ctx.search_term.yellow(), "\x1B[33m|\x1B[0m");
    } else if ctx.mode == TradeMode::InputLookingFor {
        print!(" \x1B[90mOptional:\x1B[0m \x1B[1;33mLooking for?\x1B[0m {}{} \x1B[90m(Enter to skip)\x1B[0m\x1B[K\r\n", ctx.looking_for_input.yellow(), "\x1B[33m|\x1B[0m");
    } else if ctx.mode == TradeMode::ConfirmPost {
        print!(" \x1B[1;33mPost this trade? (y/n)\x1B[0m\x1B[K\r\n");
    } else {
        print!(" \x1B[90mSelect a Pokemon to put up for trade\x1B[0m\x1B[K\r\n");
    }

    print!(" {}\x1B[K\r\n", "\x1B[90m-\x1B[0m".repeat(tw.saturating_sub(2)));

    for row in 0..list_height {
        let idx = ctx.post_scroll + row;
        let fi = if idx < filtered.len() { Some(filtered[idx]) } else { None };

        let left_cell = if let Some(pi) = fi {
            let p = &ctx.pc_pokemon[pi];
            let shiny = if p.shiny { "\x1B[1;33m*\x1B[0m" } else { " " };
            if idx == ctx.post_selected {
                format!(" \x1B[7m> {:<15}{} x{} P:{:<3}\x1B[0m", p.name, shiny, p.count, p.power)
            } else {
                format!("   \x1B[32m{:<15}\x1B[0m{} x{} \x1B[33mP:{:<3}\x1B[0m", p.name, shiny, p.count, p.power)
            }
        } else { String::new() };

        // Right panel — preview of selected pokemon
        let sel_idx = if ctx.post_selected < filtered.len() { Some(filtered[ctx.post_selected]) } else { None };
        let right_cell = if let Some(si) = sel_idx {
            let p = &ctx.pc_pokemon[si];
            match row {
                0 => format!(" \x1B[1;36m{}\x1B[0m{}", p.name, if p.shiny { " \x1B[1;33m[SHINY]\x1B[0m" } else { "" }),
                2 => { let types_str = p.types.iter().map(|t| color_type(t)).collect::<Vec<_>>().join(" / "); format!(" Type:  {}", types_str) },
                3 => format!(" Power: \x1B[33m{}\x1B[0m", p.power),
                4 => format!(" Count: {}", p.count),
                6 => {
                    if ctx.mode == TradeMode::InputLookingFor || ctx.mode == TradeMode::ConfirmPost {
                        format!(" \x1B[1mLooking for:\x1B[0m")
                    } else { String::new() }
                },
                7 => {
                    if ctx.mode == TradeMode::InputLookingFor || ctx.mode == TradeMode::ConfirmPost {
                        format!("   \x1B[33m{}\x1B[0m", ctx.looking_for_input)
                    } else { String::new() }
                },
                _ => String::new(),
            }
        } else { String::new() };

        print!("{}\x1B[{}G\x1B[90m|\x1B[0m{}\x1B[K\r\n", left_cell, left_width + 1, right_cell);
    }

    print!(" {}\x1B[K\r\n", "\x1B[90m-\x1B[0m".repeat(tw.saturating_sub(2)));
    if let Some(ref msg) = ctx.status_msg {
        print!(" \x1B[1;33m{}\x1B[0m\x1B[K", msg);
    } else if ctx.has_active_trade {
        print!(" \x1B[90mPress 3 to manage your trade | q: Quit\x1B[0m\x1B[K");
    } else if ctx.mode == TradeMode::InputLookingFor {
        print!(" \x1B[90mType what you want | Enter: Confirm | Esc: Cancel\x1B[0m\x1B[K");
    } else {
        print!(" \x1B[90m↑↓: Nav | /: Search | Enter: Select | q: Quit\x1B[0m\x1B[K");
    }
    stdout().flush()?;
    Ok(())
}

fn handle_post_input(ctx: &mut TradeContext, code: KeyCode) -> Result<bool, Box<dyn std::error::Error>> {
    let (_, th) = terminal::size()?;
    let list_height = (th as usize).saturating_sub(6);
    ctx.status_msg = None;

    if ctx.has_active_trade {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(true),
            _ => {}
        }
        return Ok(false);
    }

    if ctx.mode == TradeMode::ConfirmPost {
        if code == KeyCode::Char('y') || code == KeyCode::Char('Y') {
            // Post the trade
            let filtered: Vec<usize> = ctx.pc_pokemon.iter().enumerate()
                .filter(|(_, p)| ctx.search_term.is_empty() || p.name.to_lowercase().contains(&ctx.search_term.to_lowercase()))
                .map(|(i, _)| i).collect();
            if ctx.post_selected < filtered.len() {
                let p = &ctx.pc_pokemon[filtered[ctx.post_selected]];
                let body = serde_json::json!({
                    "offering": { "name": p.name, "types": p.types, "power_rank": p.power, "shiny": p.shiny },
                    "looking_for": ctx.looking_for_input
                }).to_string();
                match api_post("/api/trade/create", &ctx.token, &body) {
                    Some(s) => {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                            if let Some(err) = data["error"].as_str() {
                                ctx.status_msg = Some(err.to_string());
                            } else {
                                ctx.status_msg = Some(data["message"].as_str().unwrap_or("Trade posted!").to_string());
                                ctx.has_active_trade = true;
                                refresh_my_trade(ctx);
                            }
                        }
                    }
                    None => { ctx.status_msg = Some("Failed to post trade.".to_string()); }
                }
            }
            ctx.mode = TradeMode::Normal;
            ctx.looking_for_input.clear();
        } else {
            ctx.mode = TradeMode::InputLookingFor;
        }
        return Ok(false);
    }

    if ctx.mode == TradeMode::InputLookingFor {
        match code {
            KeyCode::Esc => { ctx.mode = TradeMode::Normal; ctx.looking_for_input.clear(); }
            KeyCode::Enter => {
                // Empty is fine — defaults to "open to offers"
                ctx.mode = TradeMode::ConfirmPost;
            }
            KeyCode::Backspace => { ctx.looking_for_input.pop(); }
            KeyCode::Char(c) => { ctx.looking_for_input.push(c); }
            _ => {}
        }
        return Ok(false);
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(true),
        KeyCode::Char('/') => { ctx.searching = true; ctx.search_term.clear(); }
        KeyCode::Up | KeyCode::Char('k') => { if ctx.post_selected > 0 { ctx.post_selected -= 1; } if ctx.post_selected < ctx.post_scroll { ctx.post_scroll = ctx.post_selected; } }
        KeyCode::Down | KeyCode::Char('j') => {
            if ctx.post_selected + 1 < ctx.pc_pokemon.len() { ctx.post_selected += 1; }
            if ctx.post_selected >= ctx.post_scroll + list_height { ctx.post_scroll = ctx.post_selected + 1 - list_height; }
        }
        KeyCode::Enter => {
            if !ctx.pc_pokemon.is_empty() {
                ctx.mode = TradeMode::InputLookingFor;
                ctx.looking_for_input.clear();
            }
        }
        _ => {}
    }
    Ok(false)
}

// --- My Trade Tab ---

fn render_my_trade(ctx: &TradeContext) -> Result<(), Box<dyn std::error::Error>> {
    let (tw, th) = terminal::size()?;
    let tw = tw as usize;
    let th = th as usize;
    let left_width = 40.min(tw / 2);
    let list_height = th.saturating_sub(6);

    stdout().execute(cursor::MoveTo(0, 0))?;
    render_tab_header(ctx, tw);

    if ctx.my_trade.is_none() {
        print!(" \x1B[1;33mNo active trade listing. Press 2 to post one.\x1B[0m\x1B[K\r\n");
    } else {
        let t = ctx.my_trade.as_ref().unwrap();
        print!(" \x1B[90mOffering: \x1B[32m{}\x1B[90m | Looking for: \x1B[33m{}\x1B[0m\x1B[K\r\n", t.offering_name, t.looking_for);
    }

    print!(" {}\x1B[K\r\n", "\x1B[90m-\x1B[0m".repeat(tw.saturating_sub(2)));

    for row in 0..list_height {
        let left_cell = if ctx.my_trade.is_none() {
            String::new()
        } else if ctx.my_offers.is_empty() {
            if row == 0 { " \x1B[90mNo offers yet. Check back later.\x1B[0m".to_string() }
            else { String::new() }
        } else {
            let idx = ctx.offer_scroll + row;
            if idx < ctx.my_offers.len() {
                let o = &ctx.my_offers[idx];
                let shiny = if o.pokemon_shiny { "\x1B[1;33m*\x1B[0m" } else { " " };
                if idx == ctx.offer_selected {
                    format!(" \x1B[7m> {:<15}{} from {}\x1B[0m", o.pokemon_name, shiny, o.from)
                } else {
                    format!("   \x1B[32m{:<15}\x1B[0m{} \x1B[90mfrom {}\x1B[0m", o.pokemon_name, shiny, o.from)
                }
            } else { String::new() }
        };

        // Right panel — selected offer details
        let right_cell = if !ctx.my_offers.is_empty() && ctx.offer_selected < ctx.my_offers.len() {
            let o = &ctx.my_offers[ctx.offer_selected];
            match row {
                0 => format!(" \x1B[1;36m{}\x1B[0m{}", o.pokemon_name, if o.pokemon_shiny { " \x1B[1;33m[SHINY]\x1B[0m" } else { "" }),
                2 => { let types_str = o.pokemon_types.iter().map(|t| color_type(t)).collect::<Vec<_>>().join(" / "); format!(" Type:   {}", types_str) },
                3 => format!(" Power:  \x1B[33m{}\x1B[0m", o.pokemon_power),
                4 => format!(" From:   \x1B[32m{}\x1B[0m", o.from),
                _ => String::new(),
            }
        } else if ctx.my_trade.is_some() && row == 0 {
            let t = ctx.my_trade.as_ref().unwrap();
            format!(" \x1B[90mTrade ID: {}\x1B[0m", t.id)
        } else { String::new() };

        print!("{}\x1B[{}G\x1B[90m|\x1B[0m{}\x1B[K\r\n", left_cell, left_width + 1, right_cell);
    }

    print!(" {}\x1B[K\r\n", "\x1B[90m-\x1B[0m".repeat(tw.saturating_sub(2)));
    if let Some(ref msg) = ctx.status_msg {
        print!(" \x1B[1;33m{}\x1B[0m\x1B[K", msg);
    } else if let Some(ref action) = ctx.confirming {
        print!(" \x1B[1;31m{} this? Press Y to confirm, any other key to cancel\x1B[0m\x1B[K",
            match action.as_str() { "accept" => "Accept offer", "reject" => "Reject offer", "cancel" => "Cancel trade", _ => "Confirm" });
    } else if ctx.my_trade.is_some() {
        let offers_label = if ctx.my_offers.is_empty() { "" } else { "↑↓: Nav | a: Accept | r: Reject | " };
        print!(" \x1B[90m{}c: Cancel trade | R: Refresh | q: Quit\x1B[0m\x1B[K", offers_label);
    } else {
        print!(" \x1B[90mq: Quit\x1B[0m\x1B[K");
    }
    stdout().flush()?;
    Ok(())
}

fn handle_my_trade_input(ctx: &mut TradeContext, code: KeyCode) -> Result<bool, Box<dyn std::error::Error>> {
    let (_, th) = terminal::size()?;
    let list_height = (th as usize).saturating_sub(6);
    ctx.status_msg = None;

    if ctx.my_trade.is_none() {
        if code == KeyCode::Char('q') || code == KeyCode::Char('Q') || code == KeyCode::Esc { return Ok(true); }
        return Ok(false);
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(true),
        KeyCode::Up | KeyCode::Char('k') => { if ctx.offer_selected > 0 { ctx.offer_selected -= 1; } if ctx.offer_selected < ctx.offer_scroll { ctx.offer_scroll = ctx.offer_selected; } }
        KeyCode::Down | KeyCode::Char('j') => {
            if ctx.offer_selected + 1 < ctx.my_offers.len() { ctx.offer_selected += 1; }
            if ctx.offer_selected >= ctx.offer_scroll + list_height { ctx.offer_scroll = ctx.offer_selected + 1 - list_height; }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if !ctx.my_offers.is_empty() { ctx.confirming = Some("accept".to_string()); }
            else { ctx.status_msg = Some("No offers to accept.".to_string()); }
        }
        KeyCode::Char('r') if ctx.my_offers.is_empty() => {
            ctx.status_msg = Some("No offers to reject.".to_string());
        }
        KeyCode::Char('r') => {
            ctx.confirming = Some("reject".to_string());
        }
        KeyCode::Char('R') => { refresh_my_trade(ctx); ctx.status_msg = Some("Refreshed.".to_string()); }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            ctx.confirming = Some("cancel".to_string());
        }
        _ => {}
    }
    Ok(false)
}

fn do_confirm_action(ctx: &mut TradeContext, action: &str) {
    let trade_id = ctx.my_trade.as_ref().map(|t| t.id.clone()).unwrap_or_default();

    match action {
        "accept" => {
            if ctx.offer_selected < ctx.my_offers.len() {
                let offer_id = ctx.my_offers[ctx.offer_selected].id.clone();
                let body = serde_json::json!({ "trade_id": trade_id, "offer_id": offer_id }).to_string();
                match api_post("/api/trade/accept", &ctx.token, &body) {
                    Some(s) => {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                            ctx.status_msg = Some(data["message"].as_str().unwrap_or("Trade accepted!").to_string());
                        }
                    }
                    None => { ctx.status_msg = Some("Failed to accept.".to_string()); }
                }
                refresh_my_trade(ctx);
            }
        }
        "reject" => {
            if ctx.offer_selected < ctx.my_offers.len() {
                let offer_id = ctx.my_offers[ctx.offer_selected].id.clone();
                let body = serde_json::json!({ "trade_id": trade_id, "offer_id": offer_id }).to_string();
                match api_post("/api/trade/reject", &ctx.token, &body) {
                    Some(s) => {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                            ctx.status_msg = Some(data["message"].as_str().unwrap_or("Offer rejected.").to_string());
                        }
                    }
                    None => { ctx.status_msg = Some("Failed to reject.".to_string()); }
                }
                refresh_my_trade(ctx);
            }
        }
        "cancel" => {
            let body = serde_json::json!({ "trade_id": trade_id }).to_string();
            match api_post("/api/trade/cancel", &ctx.token, &body) {
                Some(s) => {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&s) {
                        ctx.status_msg = Some(data["message"].as_str().unwrap_or("Trade cancelled.").to_string());
                    }
                }
                None => { ctx.status_msg = Some("Failed to cancel.".to_string()); }
            }
            refresh_my_trade(ctx);
        }
        _ => {}
    }
}
