mod api;
mod battle;
mod catch;
mod crypto;
mod display;
mod models;
mod pc_tui;
mod pokedex_tui;
mod setup;
mod storage;
mod trade;

use clap::{Parser, Subcommand};
use colored::*;
use std::io::{stdout, Write};

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
    #[command(long_about = "Attempt to catch a Pokemon using a Poke Ball.\n\n\
Each Pokemon has different catch rates based on rarity:\n\
- Legendary/Mythical: 3% base rate (very hard)\n\
- Pseudo-legendary/Starters: 45% base rate (hard)\n\
- Common Pokemon: 120-255% base rate (easy)\n\n\
Examples:\n\
  catch-pokemon catch pikachu\n\
  catch-pokemon catch bulbasaur --hide-pokemon\n\
  catch-pokemon catch charizard --skip-animation")]
    Catch {
        /// Name of the Pokemon to catch
        #[arg(hide = true)]
        pokemon: String,

        /// Skip the animated Pokeball throwing sequence
        #[arg(short = 's', long, help = "Skip animations for faster catching", hide = true)]
        skip_animation: bool,

        /// Hide the Pokemon ASCII art when it appears
        #[arg(long, default_value = "false", hide = true)]
        hide_pokemon: bool,

        /// Mark this Pokemon as shiny (set by encounter system)
        #[arg(long, default_value = "false", hide = true)]
        shiny: bool,

        /// Session token from encounter (required to prevent manual catching)
        #[arg(long, hide = true)]
        token: Option<String>,

        /// Attempt number for rolling flee rate (set by shell function)
        #[arg(long, default_value = "1", hide = true)]
        attempt: u32,
    },

    /// Display your Pokemon collection with detailed statistics
    #[command(long_about = "View all Pokemon you've caught in your PC storage.\n\n\
Shows detailed information including:\n\
- Pokemon grouped by name with total counts\n\
- Breakdown of catches by ball type for each Pokemon\n\
- Summary statistics of total catches by ball type\n\
- Recent catch history with timestamps\n\n\
Examples:\n\
  catch-pokemon pc\n\
  catch-pokemon pc --search\n\
  catch-pokemon pc -s")]
    Pc {
        /// Launch interactive fuzzy search interface
        #[arg(short = 's', long, help = "Launch interactive fuzzy search interface")]
        search: bool,
    },

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

    /// Verify the integrity of your PC storage
    #[command(long_about = "Verify the cryptographic integrity chain of your Pokemon storage.\n\n\
Checks that:\n\
- No Pokemon entries have been added, removed, or reordered\n\
- No entry fields have been tampered with\n\
- The chain is complete from genesis to the latest entry\n\n\
Examples:\n\
  catch-pokemon verify\n\
  catch-pokemon verify --file /path/to/pc_storage.json")]
    Verify {
        /// Path to a specific encrypted PC file to verify
        #[arg(long)]
        file: Option<String>,
    },

    /// Set up shell functions (catch, pc, pokemon_encounter, etc.)
    #[command(long_about = "Install shell functions for the Pokemon catching game.\n\n\
This sets up convenient shell commands:\n\
- catch: Attempt to catch the current wild Pokemon\n\
- pc: View your Pokemon collection\n\
- pokemon_encounter: Generate a new wild Pokemon encounter\n\
- pokemon_new: Force a new encounter\n\
- pokemon_status: Show current Pokemon status\n\
- pokemon_check <name>: Check if you own a specific Pokemon\n\
- pokemon_help: Show all available commands\n\n\
The shell functions are installed to ~/.local/share/catch-pokemon/functions.sh\n\
and automatically sourced from your shell config (.zshrc or .bashrc).\n\n\
Example:\n\
  catch-pokemon setup")]
    Setup,

    /// Update to the latest version (or a specific version)
    #[command(long_about = "Download and install the latest version of catch-pokemon.\n\n\
This fetches the latest release from GitHub and replaces the current binary.\n\
Your Pokemon collection and shell functions are preserved.\n\
Optionally pin to a specific version.\n\n\
Examples:\n\
  catch-pokemon update\n\
  catch-pokemon update --version v3.5.0")]
    Update {
        /// Pin to a specific version (e.g. v3.5.0)
        #[arg(long)]
        version: Option<String>,
    },

    /// Restore PC from a plaintext backup JSON file
    #[command(long_about = "Restore your Pokemon collection from a plaintext backup JSON file.\n\n\
This re-encrypts and re-signs the data with the current binary's key.\n\
Use this if your PC got corrupted or you're migrating to a new machine.\n\n\
The backup file is pc_backup.json in your storage directory, or you can\n\
provide a path to any backup file.\n\n\
Example:\n\
  catch-pokemon restore\n\
  catch-pokemon restore --file /path/to/pc_backup.json")]
    Restore {
        /// Path to the plaintext backup JSON file
        #[arg(long)]
        file: Option<String>,
    },

    /// Browse the Pokedex — see all Pokemon, track what you've seen and caught
    #[command(long_about = "Browse the full Pokedex with an interactive TUI.\n\n\
Shows all Pokemon with their types, power rank, and catch status.\n\
Pokemon you've encountered are marked as seen, caught ones are marked differently.\n\
Use fuzzy search to filter by name.\n\n\
Example:\n\
  catch-pokemon pokedex")]
    Pokedex,

    /// Manage your battle team (up to 20 Pokemon)
    #[command(long_about = "Manage your battle team for online battles.\n\n\
Your battle team holds up to 20 Pokemon selected from your PC.\n\
This is the roster you bring to battles — opponents see your battle team,\n\
and you pick 6 from it each round.\n\n\
The battle team is stored in an encrypted file alongside your PC.\n\n\
Examples:\n\
  catch-pokemon team                    # View your battle team\n\
  catch-pokemon team --add pikachu      # Add a Pokemon from your PC\n\
  catch-pokemon team --remove pikachu   # Remove a Pokemon from your team\n\
  catch-pokemon team --clear            # Clear the entire team")]
    Team {
        /// Add a Pokemon from your PC to the battle team
        #[arg(long)]
        add: Option<String>,

        /// Remove a Pokemon from the battle team
        #[arg(long)]
        remove: Option<String>,

        /// Clear the entire battle team
        #[arg(long)]
        clear: bool,
    },

    /// Join the battle queue and fight another trainer
    #[command(long_about = "Join the matchmaking queue and battle another trainer.\n\n\
You need at least 6 Pokemon in your PC and a battle team of up to 20.\n\
Once matched, you'll see your opponent's team and pick 6 Pokemon each round.\n\
Best of 5 rounds. Formula: 40%% power + 40%% type advantage + 20%% RNG.\n\n\
Example:\n\
  catch-pokemon battle")]
    Battle,

    /// Browse and manage trades on the bulletin board
    #[command(long_about = "Trade Pokemon with other trainers.\n\n\
Post a trade, browse open listings, make offers, or manage your trades.\n\n\
Example:\n\
  catch-pokemon trade")]
    Trade,

    /// Generate a weighted random Pokemon encounter
    #[command(long_about = "Generate a random Pokemon encounter weighted by rarity.\n\n\
Common Pokemon (high catch rate) appear more often than rare ones.\n\
Legendary and mythical Pokemon are extremely rare encounters.\n\n\
Encounter weights are based on catch rates:\n\
- Common (catch_rate 255): Very frequent\n\
- Uncommon (catch_rate 120): Moderate\n\
- Rare (catch_rate 45-75): Uncommon\n\
- Legendary/Mythical (catch_rate 3): Extremely rare\n\n\
Output modes:\n\
- Default: Prints only the Pokemon name (for scripting)\n\
- --show-pokemon: Also displays the Pokemon sprite\n\n\
Examples:\n\
  catch-pokemon encounter\n\
  catch-pokemon encounter --show-pokemon")]
    Encounter {
        /// Display the Pokemon sprite alongside the name
        #[arg(long, help = "Show the Pokemon sprite using pokemon-colorscripts")]
        show_pokemon: bool,
    },
}

fn release_pokemon(pokemon_name: String, number: usize) {
    let mut storage = models::PcStorage::load();

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
    let storage = models::PcStorage::load();

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

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Catch { pokemon, skip_animation, hide_pokemon, shiny, token, attempt } => {
            catch::catch_pokemon(pokemon, skip_animation, hide_pokemon, shiny, token, attempt);
        },
        Commands::Pc { search } => {
            pc_tui::show_pc(search);
        },
        Commands::Release { pokemon, number } => {
            release_pokemon(pokemon, number);
        },
        Commands::Status { pokemon, boolean } => {
            check_pokemon(pokemon, boolean);
        },
        Commands::Clear => {
            storage::clear_pc();
        },
        Commands::Verify { file } => {
            storage::verify_pc(file);
        },
        Commands::Setup => {
            setup::setup_shell();
        },
        Commands::Battle => {
            battle::battle_tui();
        },
        Commands::Trade => {
            trade::trade_tui();
        },
        Commands::Pokedex => {
            pokedex_tui::show_pokedex();
        },
        Commands::Restore { file } => {
            storage::restore_pc(file);
        },
        Commands::Team { add, remove, clear } => {
            setup::manage_team(add, remove, clear);
        },
        Commands::Encounter { show_pokemon } => {
            catch::encounter_pokemon(show_pokemon);
        },
        Commands::Update { version } => {
            setup::update_binary(version);
        }
    }
}
