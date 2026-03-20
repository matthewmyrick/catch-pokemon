#!/bin/bash

# Test script: Player 1 — interactive battle
# Usage: ./player-1.sh [name]

API="http://localhost:8080"
# Get GitHub token from gh CLI
TOKEN=$(gh auth token 2>/dev/null)
if [ -z "$TOKEN" ]; then
  echo -e "${RED}Not logged in to GitHub. Run: gh auth login${NC}"
  exit 1
fi
PLAYER_NAME="Matt"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Player 1's PC
PC='[
  {"name":"charizard","types":["fire","flying"],"power_rank":65,"shiny":false},
  {"name":"pikachu","types":["electric"],"power_rank":34,"shiny":false},
  {"name":"gyarados","types":["water","flying"],"power_rank":58,"shiny":false},
  {"name":"dragonite","types":["dragon","flying"],"power_rank":88,"shiny":false},
  {"name":"gengar","types":["ghost","poison"],"power_rank":55,"shiny":false},
  {"name":"lucario","types":["fighting","steel"],"power_rank":58,"shiny":false},
  {"name":"gardevoir","types":["psychic","fairy"],"power_rank":54,"shiny":false},
  {"name":"mewtwo","types":["psychic"],"power_rank":93,"shiny":true}
]'

echo -e "${CYAN}${BOLD}=== $PLAYER_NAME ===${NC}"
echo ""

# Check if server is running
echo -e "${YELLOW}Connecting to battle server...${NC}"
HEALTH=$(curl -s --connect-timeout 3 "$API/health" 2>/dev/null)
if [ -z "$HEALTH" ]; then
  echo -e "${RED}Could not connect to battle server at $API${NC}"
  echo -e "${RED}Make sure the server is running: docker compose up${NC}"
  exit 1
fi
echo -e "${GREEN}Connected.${NC}"
echo ""

# Join queue (blocks until matched)
echo -e "${YELLOW}Searching for opponent...${NC}"
MATCH=$(curl -s -X POST "$API/api/battle/join" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$PC")

STATUS=$(echo "$MATCH" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null)

if [ "$STATUS" = "timeout" ]; then
  echo -e "${YELLOW}No opponent found. Try again later.${NC}"
  exit 1
elif [ "$STATUS" != "matched" ]; then
  MSG=$(echo "$MATCH" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error', d.get('message','Unknown error')))" 2>/dev/null)
  if [ -z "$MSG" ]; then
    echo -e "${RED}Connection lost to battle server.${NC}"
  else
    echo -e "${RED}$MSG${NC}"
  fi
  exit 1
fi

BATTLE_ID=$(echo "$MATCH" | python3 -c "import sys,json; print(json.load(sys.stdin)['battle_id'])")
OPPONENT=$(echo "$MATCH" | python3 -c "import sys,json; print(json.load(sys.stdin)['opponent_id'])")

echo ""
echo -e "${GREEN}${BOLD}Opponent found: $OPPONENT${NC}"
echo -e "${CYAN}Battle ID: $BATTLE_ID${NC}"
echo ""
echo -e "${YELLOW}${BOLD}Opponent's PC:${NC}"
echo "$MATCH" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for p in data['opponent_pc']:
    shiny = ' [SHINY]' if p.get('shiny') else ''
    types = '/'.join(p['types'])
    print(f'  {p[\"name\"]:15} Power: {p[\"power_rank\"]:3}  Type: {types}{shiny}')
"
echo ""
# Start background heartbeat (polls status every 5s to keep connection alive)
heartbeat() {
  while true; do
    curl -s "$API/api/battle/status" -H "Authorization: Bearer $TOKEN" > /dev/null 2>&1
    sleep 5
  done
}
heartbeat &
HEARTBEAT_PID=$!
trap "kill $HEARTBEAT_PID 2>/dev/null" EXIT

echo -e "${CYAN}${BOLD}Your PC:${NC}"
echo "$PC" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for i, p in enumerate(data):
    shiny = ' [SHINY]' if p.get('shiny') else ''
    types = '/'.join(p['types'])
    print(f'  [{i+1}] {p[\"name\"]:15} Power: {p[\"power_rank\"]:3}  Type: {types}{shiny}')
"
echo ""

# Battle rounds (best of 5)
TIMER=120
ROUND=1

while [ "$ROUND" -le 5 ]; do
  echo -e "${CYAN}${BOLD}--- Round $ROUND (Best of 5) ---${NC}"
  START_TIME=$(date +%s)

  # Selection loop with timer and confirmation
  TEAM=""
  AUTO_SELECTED=false
  while [ -z "$TEAM" ]; do
    ELAPSED=$(( $(date +%s) - START_TIME ))
    REMAINING=$(( TIMER - ELAPSED ))

    if [ "$REMAINING" -le 0 ] || [ "$AUTO_SELECTED" = "pending" ]; then
      # Auto-select top 6 by power rank
      AUTO_SELECTED=true
      TEAM=$(echo "$PC" | python3 -c "
import sys, json
pc = json.load(sys.stdin)
ranked = sorted(enumerate(pc), key=lambda x: x[1]['power_rank'], reverse=True)[:6]
team = [p for _, p in ranked]
print(json.dumps(team))
" 2>/dev/null)
      echo -e "${RED}Time's up! Auto-selecting top 6 by power rank.${NC}"
    else
      echo -e "${YELLOW}Select 6 Pokemon by number (e.g. 1 2 3 4 5 6) [${REMAINING}s remaining]:${NC}"
      read -t "$REMAINING" -p "> " SELECTIONS

      if [ $? -ne 0 ]; then
        echo ""
        AUTO_SELECTED="pending"
        continue
      fi

      # Build team from manual selection
      TEAM=$(echo "$PC" | python3 -c "
import sys, json
pc = json.load(sys.stdin)
picks = '$SELECTIONS'.split()
team = []
seen = set()
for p in picks:
    idx = int(p) - 1
    if 0 <= idx < len(pc) and idx not in seen:
        team.append(pc[idx])
        seen.add(idx)
if len(team) != 6:
    sys.exit(1)
print(json.dumps(team))
" 2>/dev/null)

      if [ -z "$TEAM" ]; then
        echo -e "${RED}Invalid selection. Pick exactly 6 unique numbers from your PC.${NC}"
        continue
      fi
    fi

    # Show selected team
    echo ""
    echo -e "${CYAN}Your team:${NC}"
    echo "$TEAM" | python3 -c "
import sys, json
team = json.load(sys.stdin)
total_power = 0
for p in team:
    shiny = ' [SHINY]' if p.get('shiny') else ''
    types = '/'.join(p['types'])
    total_power += p['power_rank']
    print(f'  {p[\"name\"]:15} Power: {p[\"power_rank\"]:3}  Type: {types}{shiny}')
print(f'  {\"\":15} Total: {total_power}')
"

    # Skip confirmation on auto-select, ask on manual
    if [ "$AUTO_SELECTED" = "true" ]; then
      echo ""
      echo -e "${YELLOW}Auto-locked.${NC}"
    else
      echo ""
      read -p "Lock in this team? (y/n) > " CONFIRM
      if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
        TEAM=""
        echo -e "${YELLOW}Pick again.${NC}"
      fi
    fi
  done

  # Submit team
  echo -e "${YELLOW}Locking in team...${NC}"
  curl -s -X POST "$API/api/battle/select" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"battle_id\": \"$BATTLE_ID\", \"team\": $TEAM}" > /dev/null

  echo -e "${GREEN}Team locked in. Waiting for opponent...${NC}"

  # Poll for round result
  while true; do
    sleep 1
    BATTLE=$(curl -s "$API/api/battle/status" -H "Authorization: Bearer $TOKEN")

    ROUND_COUNT=$(echo "$BATTLE" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('rounds',[])))" 2>/dev/null)
    if [ "$ROUND_COUNT" -ge "$ROUND" ] 2>/dev/null; then
      echo ""
      echo "$BATTLE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
r = data['rounds'][-1]
res = r['result']

print('  Your team:     ', ', '.join(p['name'] for p in r['p1_team']))
print('  Opponent team: ', ', '.join(p['name'] for p in r['p2_team']))
print()

# Calculate win odds from the deterministic components (power + type = 80%)
p1_det = 0.4 * (res['p1_power_total'] / (res['p1_power_total'] + res['p2_power_total'])) + 0.4 * (res['p1_type_bonus'] / (res['p1_type_bonus'] + res['p2_type_bonus']))
p2_det = 0.4 * (res['p2_power_total'] / (res['p1_power_total'] + res['p2_power_total'])) + 0.4 * (res['p2_type_bonus'] / (res['p1_type_bonus'] + res['p2_type_bonus']))
# With 20% RNG as a coin flip, expected value is 0.1 each
your_odds = (p1_det + 0.1) / (p1_det + p2_det + 0.2) * 100

if your_odds >= 60:
    odds_color = '\033[32m'
elif your_odds >= 45:
    odds_color = '\033[33m'
else:
    odds_color = '\033[31m'

print(f'  Your Power: {res[\"p1_power_total\"]}  |  Opponent Power: {res[\"p2_power_total\"]}')
print(f'  Your Type Adv: {res[\"p1_type_bonus\"]:.3f}  |  Opponent Type Adv: {res[\"p2_type_bonus\"]:.3f}')
print(f'  {odds_color}\033[1mYour odds: {your_odds:.0f}%\033[0m  |  RNG Roll: {res[\"rng_roll\"]:.3f}')
print()

winner = res['winner']
if winner == 'player-1-test':
    print(f'  \033[32m\033[1mYou won this round!\033[0m')
else:
    print(f'  \033[31m\033[1mYou lost this round.\033[0m')
print(f'  Series: {data[\"p1_wins\"]}-{data[\"p2_wins\"]}')
"
      break
    fi

    BSTATUS=$(echo "$BATTLE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null)
    if [ "$BSTATUS" = "complete" ] || [ "$BSTATUS" = "abandoned" ] || [ "$BSTATUS" = "none" ]; then
      break
    fi
  done

  # Check if battle is over
  BATTLE=$(curl -s "$API/api/battle/status" -H "Authorization: Bearer $TOKEN")
  BSTATUS=$(echo "$BATTLE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null)

  if [ "$BSTATUS" = "abandoned" ]; then
    echo ""
    echo -e "${BOLD}================================${NC}"
    echo -e "${YELLOW}${BOLD}Battle abandoned. Opponent disconnected.${NC}"
    echo -e "${BOLD}================================${NC}"
    exit 0
  fi

  if [ "$BSTATUS" = "none" ]; then
    echo ""
    echo -e "${BOLD}================================${NC}"
    echo -e "${YELLOW}${BOLD}Battle ended. Opponent disconnected.${NC}"
    echo -e "${BOLD}================================${NC}"
    exit 0
  fi

  if [ "$BSTATUS" = "complete" ]; then
    echo ""
    echo -e "${BOLD}================================${NC}"
    WINNER=$(echo "$BATTLE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('winner',''))" 2>/dev/null)
    P1W=$(echo "$BATTLE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('p1_wins',0))" 2>/dev/null)
    P2W=$(echo "$BATTLE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('p2_wins',0))" 2>/dev/null)
    if [ "$WINNER" = "$TOKEN" ]; then
      echo -e "${GREEN}${BOLD}You won the battle! ($P1W-$P2W)${NC}"
    else
      echo -e "${RED}${BOLD}You lost the battle. ($P1W-$P2W)${NC}"
    fi
    echo -e "${BOLD}================================${NC}"
    exit 0
  fi

  ROUND=$((ROUND + 1))
  echo ""
done
