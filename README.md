# Catch Pokemon CLI Game

A terminal-based Pokemon catching game with weighted encounters, animated ASCII art, shiny Pokemon, and a cryptographically signed PC storage system. Every terminal session is a new encounter.

> **[Installation Guide](INSTALLATION.md)** - Get up and running in one command.

## How to Play

Every time you open a terminal, a wild Pokemon appears. Type `catch` to throw a Poke Ball. If it breaks free, try again before it flees. If you catch it, it's stored in your PC forever.

```bash
pokemon_encounter    # A wild Pokemon appears!
catch                # Throw a Poke Ball
pc                   # View your collection
```

## Encounter System

Pokemon encounters are **weighted by rarity**. Common Pokemon appear frequently, while legendaries are extremely rare.

| Category | Encounter Weight | How Often |
|----------|-----------------|-----------|
| Common | 255 | Very frequent |
| Uncommon | 90-200 | Frequent |
| Rare | 45-75 | Occasional |
| Baby | 25-50 | Uncommon |
| Starter | 45 | Uncommon |
| Starter Evolution | 15-25 | Rare |
| Pseudo-Legendary | 3-25 | Very rare |
| Legendary | 3 | Extremely rare |
| Mythical | 3 | Extremely rare |

Each encounter displays the Pokemon's **category**, **type(s)**, and ASCII sprite. Legendary and mythical encounters get special announcements.

## Catch Rates

You throw a standard Poke Ball every time. The catch chance is based on the Pokemon's base catch rate:

```
catch_chance = base_catch_rate / 255 x 100%
```

| Category | Base Catch Rate | Catch Chance |
|----------|----------------|--------------|
| Common (Pidgey, Rattata) | 255 | 100% |
| Uncommon (Pikachu) | 190 | 74.5% |
| Uncommon (Pidgeotto) | 120 | 47.1% |
| Rare (Alakazam) | 50 | 19.6% |
| Starter (Charmander) | 45 | 17.6% |
| Starter Evolution (Charizard) | 15 | 5.9% |
| Pseudo-Legendary (Dragonite) | 3 | 1.2% |
| Legendary (Mewtwo) | 3 | 1.2% |
| Mythical (Mew) | 3 | 1.2% |

## Flee Rates

If a Pokemon breaks free, it may flee. Rarer Pokemon are more likely to run. Flee rates are stored per-Pokemon in the game data and scale with evolution stage.

| Category | Flee Rate |
|----------|-----------|
| Mythical | 30% |
| Legendary | 25% |
| Pseudo-Legendary | 20% |
| Starter Evolution (final stage) | 18% |
| Starter Evolution (mid stage) | 15% |
| Rare (strong) | 14% |
| Starter | 12% |
| Rare (base) | 10% |
| Uncommon (evolved) | 6-8% |
| Uncommon (base) | 5% |
| Common | 3% |

Evolved forms flee more often than their base forms. A Charizard (18%) is much harder to hold onto than a Charmander (12%).

## Shiny Pokemon

Every encounter has a **1% chance** of being shiny. Shiny Pokemon display with alternate color sprites and are tagged `[Shiny]` in the encounter. They are recorded as shiny in your PC.

## Pokemon Types

All 1016 Pokemon have their official type(s) stored in the game data. Types are displayed during encounters with color coding:

- **fire** / **water** / **grass** / **electric** / **ice**
- **fighting** / **poison** / **ground** / **flying** / **psychic**
- **bug** / **rock** / **ghost** / **dragon** / **dark** / **steel** / **fairy** / **normal**

Dual-type Pokemon display both types (e.g., `grass / poison` for Bulbasaur).

## PC Storage & Integrity

Your caught Pokemon are stored locally with **cryptographic integrity protection**:

- Every catch is signed with **HMAC-SHA256**
- Entries are linked in a **hash chain** — inserting, deleting, or reordering entries is detected
- The signing key is **derived at build time** and **never exists in source code**
- The key is further derived per-machine using **hostname + username salt**
- **10,000 rounds of HMAC key stretching** make brute-force reversal expensive
- **Domain separation** prevents cross-protocol attacks

You cannot manually add Pokemon to your PC. The only way to add a Pokemon is to catch it through the game. Run `catch-pokemon verify` to check your chain integrity at any time.

```bash
catch-pokemon verify    # Verify PC integrity
catch-pokemon pc        # View collection (also verifies)
catch-pokemon release pidgey          # Release a Pokemon
catch-pokemon release rattata -n 5    # Release multiple
catch-pokemon status mewtwo           # Check if you own one
catch-pokemon clear                   # Start over (destructive)
```

## Animation

The catching sequence:
1. Pokemon appears with ASCII sprite (via pokemon-colorscripts)
2. "You throw a Poke Ball!" with animated ASCII pokeball
3. Ball shakes left-right-left-center (2-4 times based on catch difficulty)
4. Result: Stars (caught) or ball opens (escaped)
5. If escaped: Pokemon either stays (try again) or flees (game over)

Use `--skip-animation` for instant results.

## Building

```bash
cargo build              # Debug build
cargo build --release    # Optimized release build
cargo run -- catch pikachu  # Run directly
```

The build process generates a unique cryptographic key via `build.rs` that is embedded in the binary. See [PC Storage & Integrity](#pc-storage--integrity) for details.

## License

MIT

## Contributing

Pull requests are welcome! Ideas for improvements:
- Add more Pokeball types (Great Ball, Ultra Ball, etc.)
- Add battle system before catching
- Create trading functionality between users
- Add Pokemon stats and levels
- Add a Pokedex completion tracker

## Acknowledgments

- [pokemon-colorscripts](https://gitlab.com/phoneybadger/pokemon-colorscripts) for the amazing Pokemon ASCII art
- The Pokemon franchise for the inspiration
