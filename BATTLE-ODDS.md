# Battle Odds - Technical Breakdown

## Overview

Battles are best-of-5 rounds. Each round, both players select 6 Pokemon from their PC. The winner of each round is determined by a formula with three components:

| Component | Weight | What it measures |
|-----------|--------|-----------------|
| Power Rank | 40% | Raw strength of your team |
| Type Advantage | 40% | How well your types match up against theirs |
| RNG | 20% | Luck factor — keeps every battle unpredictable |

## The Formula

Each player receives a score calculated as:

```
score = (0.4 x power_normalized) + (0.4 x type_normalized) + (0.2 x rng)
```

The player with the higher score wins the round.

## Step 1: Power Rank (40%)

Every Pokemon has a `power_rank` value (10-100) based on its category:

| Category | Power Range | Example |
|----------|------------|---------|
| Mythical | 92-100 | Mew (95) |
| Legendary | 85-95 | Mewtwo (93) |
| Pseudo-Legendary | 78-88 | Dragonite (88) |
| Starter Evolution (final) | 62-72 | Charizard (65) |
| Rare (strong) | 58-68 | Lucario (58) |
| Rare (mid) | 50-60 | Eevee (54) |
| Starter Evolution (mid) | 45-55 | Charmeleon (50) |
| Starter | 35-48 | Charmander (42) |
| Uncommon | 25-45 | Pikachu (34) |
| Baby | 12-25 | Pichu (18) |
| Common | 10-28 | Pidgey (16) |

**Team power** is the sum of all 6 Pokemon's power ranks:

```
team_power = sum of each Pokemon's power_rank
```

**Normalization** converts team power to a 0-1 scale relative to the opponent:

```
p1_power_norm = p1_team_power / (p1_team_power + p2_team_power)
p2_power_norm = p2_team_power / (p1_team_power + p2_team_power)
```

### Example

Player 1 team power: 417 (Charizard 65 + Dragonite 88 + Mewtwo 93 + Lucario 58 + Gengar 55 + Gyarados 58)

Player 2 team power: 387 (Tyranitar 82 + Salamence 80 + Blastoise 68 + Alakazam 55 + Machamp 52 + Arcanine 50)

```
P1 power_norm = 417 / (417 + 387) = 0.519
P2 power_norm = 387 / (417 + 387) = 0.481
```

Player 1 has a slight power advantage (51.9% vs 48.1%).

## Step 2: Type Advantage (40%)

Type advantage is calculated by checking every attacker-defender matchup between the two teams using the standard Pokemon type chart.

### Type Effectiveness

| Multiplier | Meaning |
|-----------|---------|
| 1.2x | Super effective (attacking type is strong against defending type) |
| 1.0x | Neutral (no special interaction) |
| 0.8x | Not very effective (attacking type is weak against defending type) |

### Full Type Chart

```
fire     -> strong vs: grass, ice, bug, steel       | weak vs: water, fire, rock, dragon
water    -> strong vs: fire, ground, rock            | weak vs: water, grass, dragon
grass    -> strong vs: water, ground, rock           | weak vs: fire, grass, poison, flying, bug, dragon, steel
electric -> strong vs: water, flying                 | weak vs: electric, grass, dragon
ice      -> strong vs: grass, ground, flying, dragon | weak vs: fire, water, ice, steel
fighting -> strong vs: normal, ice, rock, dark, steel| weak vs: poison, flying, psychic, bug, fairy
poison   -> strong vs: grass, fairy                  | weak vs: poison, ground, rock, ghost
ground   -> strong vs: fire, electric, poison, rock, steel | weak vs: grass, bug
flying   -> strong vs: grass, fighting, bug          | weak vs: electric, rock, steel
psychic  -> strong vs: fighting, poison              | weak vs: psychic, steel
bug      -> strong vs: grass, psychic, dark          | weak vs: fire, fighting, poison, flying, ghost, steel, fairy
rock     -> strong vs: fire, ice, flying, bug        | weak vs: fighting, ground, steel
ghost    -> strong vs: psychic, ghost                | weak vs: dark
dragon   -> strong vs: dragon                        | weak vs: steel
dark     -> strong vs: psychic, ghost                | weak vs: fighting, dark, fairy
steel    -> strong vs: ice, rock, fairy              | weak vs: fire, water, electric, steel
fairy    -> strong vs: fighting, dragon, dark        | weak vs: fire, poison, steel
normal   -> strong vs: (none)                        | weak vs: rock, steel
```

### Calculation

For each attacker on Team 1 vs each defender on Team 2, every attacking type is checked against every defending type:

```
for each attacker in team1:
    for each defender in team2:
        for each atk_type in attacker.types:
            for each def_type in defender.types:
                lookup multiplier from type chart (default 1.0 if not listed)
                add to running total
                increment matchup count

type_advantage = total / matchup_count
```

This produces an average effectiveness across all matchups. A team with favorable types scores above 1.0, unfavorable below 1.0.

**Normalization** (same approach as power):

```
p1_type_norm = p1_type_advantage / (p1_type_advantage + p2_type_advantage)
p2_type_norm = p2_type_advantage / (p1_type_advantage + p2_type_advantage)
```

### Example

Player 1 (fire, dragon, psychic, ghost, water, fighting) vs Player 2 (rock, dark, water, psychic, fighting, fire):

```
P1 type_advantage = 0.995 (slightly below neutral — rock resists fire)
P2 type_advantage = 0.988 (slightly below neutral — psychic resists fighting)

P1 type_norm = 0.995 / (0.995 + 0.988) = 0.502
P2 type_norm = 0.988 / (0.995 + 0.988) = 0.498
```

Nearly even — but Player 1 has a tiny type edge.

## Step 3: RNG (20%)

A single random number `r` is rolled between 0.0 and 1.0:

```
p1_rng = r
p2_rng = 1.0 - r
```

This is a zero-sum coin flip. If `r = 0.8`, Player 1 gets 0.8 and Player 2 gets 0.2. If `r = 0.3`, Player 1 gets 0.3 and Player 2 gets 0.7.

The 20% weight means RNG can swing a close match but rarely overrides a dominant team.

## Final Score

```
p1_score = (0.4 x p1_power_norm) + (0.4 x p1_type_norm) + (0.2 x p1_rng)
p2_score = (0.4 x p2_power_norm) + (0.4 x p2_type_norm) + (0.2 x p2_rng)
```

Higher score wins the round.

### Full Example

```
Power:  P1 = 417, P2 = 387
  P1 power_norm = 0.519
  P2 power_norm = 0.481

Type:   P1 = 0.995, P2 = 0.988
  P1 type_norm = 0.502
  P2 type_norm = 0.498

RNG:    roll = 0.372
  P1 rng = 0.372
  P2 rng = 0.628

Final:
  P1 score = (0.4 x 0.519) + (0.4 x 0.502) + (0.2 x 0.372)
           = 0.208 + 0.201 + 0.074
           = 0.483

  P2 score = (0.4 x 0.481) + (0.4 x 0.498) + (0.2 x 0.628)
           = 0.192 + 0.199 + 0.126
           = 0.517

Winner: Player 2 (0.517 > 0.483)
```

Player 1 had more power and a slight type edge, but the RNG roll (0.372) favored Player 2 enough to flip the result.

## Win Probability

Before RNG, each player's deterministic advantage is:

```
p1_base = (0.4 x p1_power_norm) + (0.4 x p1_type_norm)
p2_base = (0.4 x p2_power_norm) + (0.4 x p2_type_norm)
```

Since RNG is a uniform 0-1 distribution weighted at 20%, the expected win probability is:

```
p1_win_prob = (p1_base + 0.1) / (p1_base + p2_base + 0.2) x 100%
```

This is displayed after each round as "Your odds: X%".

### What the odds feel like

| Your odds | What it means |
|-----------|--------------|
| 70%+ | Dominant advantage — strong team + good type matchup |
| 55-70% | Solid edge — you should win most of these |
| 45-55% | Coin flip — could go either way |
| 30-45% | Uphill battle — you need RNG luck |
| Below 30% | Major disadvantage — but upsets happen (20% is RNG) |

## Match Format

- **Best of 5** — first player to win 3 rounds wins the match
- **New team each round** — you pick 6 Pokemon again each round
- **Teams revealed after each round** — you see what your opponent picked, so you can adapt
- **2-minute selection timer** — if time runs out, your top 6 by power rank are auto-selected
- **Disconnect forfeit** — if your opponent disconnects (no heartbeat for 15s), you win

## Strategic Considerations

1. **Power stacking** — picking your 6 strongest maximizes the power component, but may leave you vulnerable on types
2. **Type counter-picking** — after seeing your opponent's PC, pick types that are super effective against theirs
3. **Mind games** — your opponent sees your PC too and may predict your picks
4. **RNG factor** — even with 30% odds, the 20% RNG means upsets happen roughly 1 in 5 times
5. **Round adaptation** — after each round, both teams are revealed so you can adjust strategy
6. **Diversity** — a team with varied types covers more matchups and is harder to counter
