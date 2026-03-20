#!/bin/bash

# Make an offer on a trade listing
# Usage: ./trade-offer.sh <trade_id>

API="http://localhost:8080"
TOKEN=$(gh auth token 2>/dev/null)
TRADE_ID="$1"

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

if [ -z "$TRADE_ID" ]; then
  echo -e "${RED}Usage: ./trade-offer.sh <trade_id>${NC}"
  echo -e "${YELLOW}Browse trades first: ./trade-browse.sh${NC}"
  exit 1
fi

echo -e "${CYAN}${BOLD}=== Make a Trade Offer ===${NC}"
echo ""

# Show trade details
echo -e "${YELLOW}Trade details:${NC}"
TRADE=$(curl -s "$API/api/trade?id=$TRADE_ID" -H "Authorization: Bearer $TOKEN")
echo "$TRADE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
if 'error' in data:
    print(f'\033[31m{data[\"error\"]}\033[0m')
    sys.exit(1)
t = data['trade']
shiny = ' [SHINY]' if t['offering'].get('shiny') else ''
print(f'  Offering: \033[32m{t[\"offering\"][\"name\"]}{shiny}\033[0m')
print(f'  Looking for: \033[33m{t[\"looking_for\"]}\033[0m')
print(f'  Posted by: \033[36m{t[\"poster_id\"]}\033[0m')

offers = data.get('offers', [])
if offers:
    print(f'  Current offers: {len(offers)}')
"

echo ""
echo -e "${CYAN}What Pokemon will you offer?${NC}"
read -p "Pokemon name: " POKEMON_NAME
read -p "Is it shiny? (y/n): " IS_SHINY

SHINY=false
if [[ "$IS_SHINY" == "y" || "$IS_SHINY" == "Y" ]]; then
  SHINY=true
fi

echo ""
echo -e "${YELLOW}Submitting offer...${NC}"
RESULT=$(curl -s -X POST "$API/api/trade/offer" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"trade_id\": \"$TRADE_ID\",
    \"pokemon\": {\"name\": \"$POKEMON_NAME\", \"types\": [], \"power_rank\": 0, \"shiny\": $SHINY}
  }")

echo "$RESULT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
if 'error' in data:
    print(f'\033[31m{data[\"error\"]}\033[0m')
else:
    print(f'\033[32m\033[1m{data[\"message\"]}\033[0m')
    print(f'  Offer ID: {data[\"offer\"][\"id\"]}')
"
