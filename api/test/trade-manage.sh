#!/bin/bash

# Manage your trade listing — view offers, accept/reject
# Usage: ./trade-manage.sh

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

echo -e "${CYAN}${BOLD}=== Your Trade Listing ===${NC}"
echo ""

RESULT=$(curl -s "$API/api/trade/mine" -H "Authorization: Bearer $TOKEN")
STATUS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null)

if [ "$STATUS" = "none" ]; then
  echo -e "${YELLOW}No active trade listing.${NC}"
  echo -e "Post one with: ./trade-post.sh"
  exit 0
fi

echo "$RESULT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
t = data['trade']
offers = data.get('offers', [])
shiny = ' [SHINY]' if t['offering'].get('shiny') else ''

print(f'  Offering: \033[32m{t[\"offering\"][\"name\"]}{shiny}\033[0m')
print(f'  Looking for: \033[33m{t[\"looking_for\"]}\033[0m')
print(f'  Trade ID: \033[90m{t[\"id\"]}\033[0m')
print()

pending = [o for o in offers if o['status'] == 'pending']
if not pending:
    print('\033[33mNo offers yet.\033[0m')
else:
    print(f'\033[36m{len(pending)} pending offer(s):\033[0m')
    print()
    for i, o in enumerate(pending):
        oshiny = ' [SHINY]' if o['pokemon'].get('shiny') else ''
        print(f'  [{i+1}] \033[32m{o[\"pokemon\"][\"name\"]}{oshiny}\033[0m from \033[36m{o[\"offer_by_id\"]}\033[0m')
        print(f'      \033[90mOffer ID: {o[\"id\"]}\033[0m')
        print()
"

TRADE_ID=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['trade']['id'])" 2>/dev/null)
PENDING=$(echo "$RESULT" | python3 -c "import sys,json; offers=[o for o in json.load(sys.stdin).get('offers',[]) if o['status']=='pending']; print(len(offers))" 2>/dev/null)

if [ "$PENDING" -gt 0 ] 2>/dev/null; then
  echo -e "${CYAN}What would you like to do?${NC}"
  echo "  a) Accept an offer"
  echo "  r) Reject an offer"
  echo "  c) Cancel the whole trade"
  echo "  q) Quit"
  read -p "> " ACTION

  case "$ACTION" in
    a|A)
      read -p "Enter the Offer ID to accept: " OFFER_ID
      ACCEPT=$(curl -s -X POST "$API/api/trade/accept" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"trade_id\": \"$TRADE_ID\", \"offer_id\": \"$OFFER_ID\"}")
      echo "$ACCEPT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
if 'error' in d:
    print(f'\033[31m{d[\"error\"]}\033[0m')
else:
    print(f'\033[32m\033[1m{d[\"message\"]}\033[0m')
"
      ;;
    r|R)
      read -p "Enter the Offer ID to reject: " OFFER_ID
      REJECT=$(curl -s -X POST "$API/api/trade/reject" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"trade_id\": \"$TRADE_ID\", \"offer_id\": \"$OFFER_ID\"}")
      echo "$REJECT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
if 'error' in d:
    print(f'\033[31m{d[\"error\"]}\033[0m')
else:
    print(f'\033[32m{d[\"message\"]}\033[0m')
"
      ;;
    c|C)
      CANCEL=$(curl -s -X POST "$API/api/trade/cancel" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"trade_id\": \"$TRADE_ID\"}")
      echo "$CANCEL" | python3 -c "
import sys, json
d = json.load(sys.stdin)
if 'error' in d:
    print(f'\033[31m{d[\"error\"]}\033[0m')
else:
    print(f'\033[32m{d[\"message\"]}\033[0m')
"
      ;;
    *)
      echo "Done."
      ;;
  esac
fi
