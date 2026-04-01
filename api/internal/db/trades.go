package db

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"time"

	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

type DBTrade struct {
	ID           string    `json:"id"`
	PosterID     string    `json:"poster_id"`
	OfferingName string    `json:"offering_name"`
	OfferingTypes []string `json:"offering_types"`
	OfferingPower int      `json:"offering_power"`
	OfferingShiny bool     `json:"offering_shiny"`
	LookingFor   string    `json:"looking_for"`
	Status       string    `json:"status"`
	CreatedAt    time.Time `json:"created_at"`
	ExpiresAt    time.Time `json:"expires_at"`
}

type DBTradeOffer struct {
	ID           string   `json:"id"`
	TradeID      string   `json:"trade_id"`
	OfferByID    string   `json:"offer_by_id"`
	PokemonName  string   `json:"pokemon_name"`
	PokemonTypes []string `json:"pokemon_types"`
	PokemonPower int      `json:"pokemon_power"`
	PokemonShiny bool     `json:"pokemon_shiny"`
	Status       string   `json:"status"`
	CreatedAt    time.Time `json:"created_at"`
}

func genID() string {
	b := make([]byte, 8)
	rand.Read(b)
	return hex.EncodeToString(b)
}

// CreateTrade posts a new trade. One active trade per user.
func CreateTrade(posterID string, offering models.Pokemon, lookingFor string) (*DBTrade, error) {
	if DB == nil {
		return nil, fmt.Errorf("database not available")
	}

	// Check if user already has an open trade
	var count int
	err := DB.QueryRow("SELECT count(*) FROM trades WHERE poster_id = $1 AND status = 'open'", posterID).Scan(&count)
	if err != nil {
		return nil, fmt.Errorf("db error: %w", err)
	}
	if count > 0 {
		return nil, fmt.Errorf("you already have an active trade listing")
	}

	id := genID()
	typesJSON, _ := json.Marshal(offering.Types)
	now := time.Now()
	expires := now.Add(72 * time.Hour) // 3 days

	_, err = DB.Exec(`INSERT INTO trades (id, poster_id, offering_name, offering_types, offering_power, offering_shiny, looking_for, status, created_at, expires_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, 'open', $8, $9)`,
		id, posterID, offering.Name, string(typesJSON), offering.PowerRank, offering.Shiny, lookingFor, now, expires)
	if err != nil {
		return nil, fmt.Errorf("insert trade: %w", err)
	}

	return &DBTrade{
		ID: id, PosterID: posterID,
		OfferingName: offering.Name, OfferingTypes: offering.Types,
		OfferingPower: offering.PowerRank, OfferingShiny: offering.Shiny,
		LookingFor: lookingFor, Status: "open",
		CreatedAt: now, ExpiresAt: expires,
	}, nil
}

// GetOpenTrades returns all open trades (not expired).
func GetOpenTrades() ([]DBTrade, error) {
	if DB == nil {
		return nil, fmt.Errorf("database not available")
	}

	// Expire old trades first
	DB.Exec("UPDATE trades SET status = 'expired' WHERE status = 'open' AND expires_at < NOW()")

	rows, err := DB.Query(`SELECT id, poster_id, offering_name, offering_types, offering_power, offering_shiny, looking_for, status, created_at, expires_at
		FROM trades WHERE status = 'open' ORDER BY created_at DESC`)
	if err != nil {
		return nil, fmt.Errorf("query trades: %w", err)
	}
	defer rows.Close()

	var trades []DBTrade
	for rows.Next() {
		var t DBTrade
		var typesJSON string
		err := rows.Scan(&t.ID, &t.PosterID, &t.OfferingName, &typesJSON, &t.OfferingPower, &t.OfferingShiny, &t.LookingFor, &t.Status, &t.CreatedAt, &t.ExpiresAt)
		if err != nil {
			continue
		}
		json.Unmarshal([]byte(typesJSON), &t.OfferingTypes)
		trades = append(trades, t)
	}
	return trades, nil
}

// GetTrade returns a single trade by ID.
func GetTrade(tradeID string) (*DBTrade, error) {
	if DB == nil {
		return nil, fmt.Errorf("database not available")
	}

	var t DBTrade
	var typesJSON string
	err := DB.QueryRow(`SELECT id, poster_id, offering_name, offering_types, offering_power, offering_shiny, looking_for, status, created_at, expires_at
		FROM trades WHERE id = $1`, tradeID).Scan(&t.ID, &t.PosterID, &t.OfferingName, &typesJSON, &t.OfferingPower, &t.OfferingShiny, &t.LookingFor, &t.Status, &t.CreatedAt, &t.ExpiresAt)
	if err != nil {
		return nil, fmt.Errorf("trade not found")
	}
	json.Unmarshal([]byte(typesJSON), &t.OfferingTypes)
	return &t, nil
}

// GetUserTrade returns the user's active open trade.
func GetUserTrade(userID string) (*DBTrade, error) {
	if DB == nil {
		return nil, fmt.Errorf("database not available")
	}

	var t DBTrade
	var typesJSON string
	err := DB.QueryRow(`SELECT id, poster_id, offering_name, offering_types, offering_power, offering_shiny, looking_for, status, created_at, expires_at
		FROM trades WHERE poster_id = $1 AND status = 'open' LIMIT 1`, userID).Scan(&t.ID, &t.PosterID, &t.OfferingName, &typesJSON, &t.OfferingPower, &t.OfferingShiny, &t.LookingFor, &t.Status, &t.CreatedAt, &t.ExpiresAt)
	if err != nil {
		return nil, err
	}
	json.Unmarshal([]byte(typesJSON), &t.OfferingTypes)
	return &t, nil
}

// MakeOffer creates an offer on a trade.
func MakeOffer(tradeID, offerByID string, pokemon models.Pokemon) (*DBTradeOffer, error) {
	if DB == nil {
		return nil, fmt.Errorf("database not available")
	}

	// Check trade exists and is open
	trade, err := GetTrade(tradeID)
	if err != nil {
		return nil, fmt.Errorf("trade not found")
	}
	if trade.Status != "open" {
		return nil, fmt.Errorf("trade is no longer open")
	}
	if trade.PosterID == offerByID {
		return nil, fmt.Errorf("you can't offer on your own trade")
	}

	// Check for existing pending offer from this user
	var existing int
	DB.QueryRow("SELECT count(*) FROM trade_offers WHERE trade_id = $1 AND offer_by_id = $2 AND status = 'pending'", tradeID, offerByID).Scan(&existing)
	if existing > 0 {
		return nil, fmt.Errorf("you already have a pending offer on this trade")
	}

	id := genID()
	typesJSON, _ := json.Marshal(pokemon.Types)
	now := time.Now()

	_, err = DB.Exec(`INSERT INTO trade_offers (id, trade_id, offer_by_id, pokemon_name, pokemon_types, pokemon_power, pokemon_shiny, status, created_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', $8)`,
		id, tradeID, offerByID, pokemon.Name, string(typesJSON), pokemon.PowerRank, pokemon.Shiny, now)
	if err != nil {
		return nil, fmt.Errorf("insert offer: %w", err)
	}

	return &DBTradeOffer{
		ID: id, TradeID: tradeID, OfferByID: offerByID,
		PokemonName: pokemon.Name, PokemonTypes: pokemon.Types,
		PokemonPower: pokemon.PowerRank, PokemonShiny: pokemon.Shiny,
		Status: "pending", CreatedAt: now,
	}, nil
}

// GetOffers returns all offers for a trade.
func GetOffers(tradeID string) ([]DBTradeOffer, error) {
	if DB == nil {
		return nil, fmt.Errorf("database not available")
	}

	rows, err := DB.Query(`SELECT id, trade_id, offer_by_id, pokemon_name, pokemon_types, pokemon_power, pokemon_shiny, status, created_at
		FROM trade_offers WHERE trade_id = $1 ORDER BY created_at DESC`, tradeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var offers []DBTradeOffer
	for rows.Next() {
		var o DBTradeOffer
		var typesJSON string
		err := rows.Scan(&o.ID, &o.TradeID, &o.OfferByID, &o.PokemonName, &typesJSON, &o.PokemonPower, &o.PokemonShiny, &o.Status, &o.CreatedAt)
		if err != nil {
			continue
		}
		json.Unmarshal([]byte(typesJSON), &o.PokemonTypes)
		offers = append(offers, o)
	}
	return offers, nil
}

// AcceptOffer accepts one offer and rejects all others. Closes the trade.
func AcceptOffer(tradeID, offerID, userID string) error {
	if DB == nil {
		return fmt.Errorf("database not available")
	}

	trade, err := GetTrade(tradeID)
	if err != nil {
		return fmt.Errorf("trade not found")
	}
	if trade.PosterID != userID {
		return fmt.Errorf("only the trade poster can accept offers")
	}
	if trade.Status != "open" {
		return fmt.Errorf("trade is no longer open")
	}

	// Accept the offer
	_, err = DB.Exec("UPDATE trade_offers SET status = 'accepted' WHERE id = $1 AND status = 'pending'", offerID)
	if err != nil {
		return fmt.Errorf("accept offer: %w", err)
	}

	// Reject all other pending offers
	DB.Exec("UPDATE trade_offers SET status = 'rejected' WHERE trade_id = $1 AND id != $2 AND status = 'pending'", tradeID, offerID)

	// Close the trade
	DB.Exec("UPDATE trades SET status = 'accepted' WHERE id = $1", tradeID)

	return nil
}

// RejectOffer rejects a single offer.
func RejectOffer(tradeID, offerID, userID string) error {
	if DB == nil {
		return fmt.Errorf("database not available")
	}

	trade, err := GetTrade(tradeID)
	if err != nil {
		return fmt.Errorf("trade not found")
	}
	if trade.PosterID != userID {
		return fmt.Errorf("only the trade poster can reject offers")
	}

	_, err = DB.Exec("UPDATE trade_offers SET status = 'rejected' WHERE id = $1 AND trade_id = $2 AND status = 'pending'", offerID, tradeID)
	if err != nil {
		return fmt.Errorf("reject offer: %w", err)
	}
	return nil
}

// CancelTrade cancels a trade and rejects all pending offers.
func CancelTrade(tradeID, userID string) error {
	if DB == nil {
		return fmt.Errorf("database not available")
	}

	trade, err := GetTrade(tradeID)
	if err != nil {
		return fmt.Errorf("trade not found")
	}
	if trade.PosterID != userID {
		return fmt.Errorf("only the trade poster can cancel")
	}
	if trade.Status != "open" {
		return fmt.Errorf("trade is not open")
	}

	DB.Exec("UPDATE trade_offers SET status = 'rejected' WHERE trade_id = $1 AND status = 'pending'", tradeID)
	DB.Exec("UPDATE trades SET status = 'cancelled' WHERE id = $1", tradeID)

	return nil
}
