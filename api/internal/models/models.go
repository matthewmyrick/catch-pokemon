package models

import "time"

type User struct {
	ID        string    `json:"id" db:"id"`
	Email     string    `json:"email" db:"email"`
	Name      string    `json:"name" db:"name"`
	Wins      int       `json:"wins" db:"wins"`
	Losses    int       `json:"losses" db:"losses"`
	Rating    int       `json:"rating" db:"rating"`
	CreatedAt time.Time `json:"created_at" db:"created_at"`
}

// Trade represents a Pokemon listing on the bulletin board
type Trade struct {
	ID          string    `json:"id"`
	PosterID    string    `json:"poster_id"`
	Offering    Pokemon   `json:"offering"`     // Pokemon being offered
	LookingFor  string    `json:"looking_for"`  // What they want (free text, e.g. "any legendary" or "charizard")
	Status      string    `json:"status"`       // open, accepted, expired, cancelled
	CreatedAt   time.Time `json:"created_at"`
	ExpiresAt   time.Time `json:"expires_at"`
}

// TradeOffer represents someone offering a Pokemon for a trade listing
type TradeOffer struct {
	ID        string    `json:"id"`
	TradeID   string    `json:"trade_id"`
	OfferByID string    `json:"offer_by_id"`
	Pokemon   Pokemon   `json:"pokemon"`
	Status    string    `json:"status"` // pending, accepted, rejected
	CreatedAt time.Time `json:"created_at"`
}

type Pokemon struct {
	Name      string   `json:"name"`
	Types     []string `json:"types"`
	PowerRank int      `json:"power_rank"`
	Shiny     bool     `json:"shiny"`
}

type BattleTeam struct {
	UserID  string    `json:"user_id"`
	Pokemon []Pokemon `json:"pokemon"`
}

type Battle struct {
	ID        string      `json:"id" db:"id"`
	Player1ID string      `json:"player1_id" db:"player1_id"`
	Player2ID string      `json:"player2_id" db:"player2_id"`
	Status    string      `json:"status" db:"status"` // waiting, selecting, battling, complete
	Round     int         `json:"round" db:"round"`
	P1Wins    int         `json:"p1_wins" db:"p1_wins"`
	P2Wins    int         `json:"p2_wins" db:"p2_wins"`
	CreatedAt time.Time   `json:"created_at" db:"created_at"`
}

type BattleRound struct {
	ID        string    `json:"id" db:"id"`
	BattleID  string    `json:"battle_id" db:"battle_id"`
	Round     int       `json:"round" db:"round"`
	P1Team    string    `json:"p1_team" db:"p1_team"`   // JSON array of pokemon names
	P2Team    string    `json:"p2_team" db:"p2_team"`   // JSON array of pokemon names
	Winner    string    `json:"winner" db:"winner"`      // player1_id or player2_id
	CreatedAt time.Time `json:"created_at" db:"created_at"`
}

// TypeChart holds effectiveness multipliers
// 1.2 = super effective, 0.8 = not very effective, 1.0 = neutral
var TypeChart = map[string]map[string]float64{
	"fire":     {"grass": 1.2, "water": 0.8, "ice": 1.2, "bug": 1.2, "steel": 1.2, "fire": 0.8, "rock": 0.8, "dragon": 0.8},
	"water":    {"fire": 1.2, "ground": 1.2, "rock": 1.2, "water": 0.8, "grass": 0.8, "dragon": 0.8},
	"grass":    {"water": 1.2, "ground": 1.2, "rock": 1.2, "fire": 0.8, "grass": 0.8, "poison": 0.8, "flying": 0.8, "bug": 0.8, "dragon": 0.8, "steel": 0.8},
	"electric": {"water": 1.2, "flying": 1.2, "electric": 0.8, "grass": 0.8, "dragon": 0.8},
	"ice":      {"grass": 1.2, "ground": 1.2, "flying": 1.2, "dragon": 1.2, "fire": 0.8, "water": 0.8, "ice": 0.8, "steel": 0.8},
	"fighting": {"normal": 1.2, "ice": 1.2, "rock": 1.2, "dark": 1.2, "steel": 1.2, "poison": 0.8, "flying": 0.8, "psychic": 0.8, "bug": 0.8, "fairy": 0.8},
	"poison":   {"grass": 1.2, "fairy": 1.2, "poison": 0.8, "ground": 0.8, "rock": 0.8, "ghost": 0.8},
	"ground":   {"fire": 1.2, "electric": 1.2, "poison": 1.2, "rock": 1.2, "steel": 1.2, "grass": 0.8, "bug": 0.8},
	"flying":   {"grass": 1.2, "fighting": 1.2, "bug": 1.2, "electric": 0.8, "rock": 0.8, "steel": 0.8},
	"psychic":  {"fighting": 1.2, "poison": 1.2, "psychic": 0.8, "steel": 0.8},
	"bug":      {"grass": 1.2, "psychic": 1.2, "dark": 1.2, "fire": 0.8, "fighting": 0.8, "poison": 0.8, "flying": 0.8, "ghost": 0.8, "steel": 0.8, "fairy": 0.8},
	"rock":     {"fire": 1.2, "ice": 1.2, "flying": 1.2, "bug": 1.2, "fighting": 0.8, "ground": 0.8, "steel": 0.8},
	"ghost":    {"psychic": 1.2, "ghost": 1.2, "dark": 0.8},
	"dragon":   {"dragon": 1.2, "steel": 0.8},
	"dark":     {"psychic": 1.2, "ghost": 1.2, "fighting": 0.8, "dark": 0.8, "fairy": 0.8},
	"steel":    {"ice": 1.2, "rock": 1.2, "fairy": 1.2, "fire": 0.8, "water": 0.8, "electric": 0.8, "steel": 0.8},
	"fairy":    {"fighting": 1.2, "dragon": 1.2, "dark": 1.2, "fire": 0.8, "poison": 0.8, "steel": 0.8},
	"normal":   {"rock": 0.8, "steel": 0.8},
}
