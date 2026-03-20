#!/bin/bash

# Post a trade listing on the bulletin board
# Usage: ./trade-post.sh

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

echo -e "${CYAN}${BOLD}=== Post a Trade ===${NC}"
echo ""

# Check for existing trade
echo -e "${YELLOW}Checking for existing listing...${NC}"
EXISTING=$(curl -s "$API/api/trade/mine" -H "Authorization: Bearer $TOKEN")
STATUS=$(echo "$EXISTING" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null)

if [ "$STATUS" != "none" ]; then
  echo -e "${YELLOW}You already have an active trade:${NC}"
  echo "$EXISTING" | python3 -m json.tool 2>/dev/null
  echo ""
  echo -e "${YELLOW}Cancel it first with: ./trade-cancel.sh${NC}"
  exit 1
fi

echo -e "${GREEN}No existing listing.${NC}"
echo ""

# Post a trade
echo -e "${CYAN}What Pokemon are you offering?${NC}"
read -p "Pokemon name: " POKEMON_NAME
read -p "Is it shiny? (y/n): " IS_SHINY

SHINY=false
if [[ "$IS_SHINY" == "y" || "$IS_SHINY" == "Y" ]]; then
  SHINY=true
fi

echo ""
echo -e "${CYAN}What are you looking for?${NC}"
echo -e "${YELLOW}(e.g. 'any legendary', 'charizard', 'any shiny', 'dragonite or salamence')${NC}"
read -p "> " LOOKING_FOR

echo ""
echo -e "${YELLOW}Posting trade...${NC}"
RESULT=$(curl -s -X POST "$API/api/trade/create" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"offering\": {\"name\": \"$POKEMON_NAME\", \"types\": [], \"power_rank\": 0, \"shiny\": $SHINY},
    \"looking_for\": \"$LOOKING_FOR\"
  }")

echo "$RESULT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
if 'error' in data:
    print(f'\033[31m{data[\"error\"]}\033[0m')
else:
    t = data['trade']
    shiny = ' [SHINY]' if t['offering']['shiny'] else ''
    print(f'\033[32m\033[1m{data[\"message\"]}\033[0m')
    print(f'  Trade ID: {t[\"id\"]}')
    print(f'  Offering: {t[\"offering\"][\"name\"]}{shiny}')
    print(f'  Looking for: {t[\"looking_for\"]}')
    print(f'  Expires: {t[\"expires_at\"]}')
"
