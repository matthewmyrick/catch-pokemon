#!/bin/bash

# Browse open trades on the bulletin board
# Usage: ./trade-browse.sh

API="http://localhost:8080"
TOKEN=$(gh auth token 2>/dev/null)

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

if [ -z "$TOKEN" ]; then
  echo -e "${RED}Not logged in to GitHub. Run: gh auth login${NC}"
  exit 1
fi

echo -e "${CYAN}${BOLD}=== Trade Bulletin Board ===${NC}"
echo ""

RESULT=$(curl -s "$API/api/trades" -H "Authorization: Bearer $TOKEN")

echo "$RESULT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
trades = data.get('trades', [])

if not trades:
    print('\033[33mNo open trades right now.\033[0m')
    sys.exit(0)

print(f'\033[36m{len(trades)} open trade(s):\033[0m')
print()

for i, t in enumerate(trades):
    shiny = ' [SHINY]' if t['offering'].get('shiny') else ''
    print(f'  [{i+1}] \033[32m{t[\"offering\"][\"name\"]}{shiny}\033[0m')
    print(f'      Posted by: \033[36m{t[\"poster_id\"]}\033[0m')
    print(f'      Looking for: \033[33m{t[\"looking_for\"]}\033[0m')
    print(f'      \033[90mTrade ID: {t[\"id\"]} | Expires: {t[\"expires_at\"][:10]}\033[0m')
    print()
"

echo -e "${YELLOW}To make an offer: ./trade-offer.sh <trade_id>${NC}"
