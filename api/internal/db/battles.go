package db

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log"
	"math"

	"github.com/matthewmyrick/catch-pokemon/api/internal/battle"
)

// PersistCompletedBattle writes a finished battle and all its rounds to the DB,
// and updates both players' wins/losses/rating in a single transaction.
func PersistCompletedBattle(b *battle.ActiveBattle) error {
	if DB == nil {
		return nil
	}

	winnerID := b.Player1
	loserID := b.Player2
	if b.P2Wins > b.P1Wins {
		winnerID = b.Player2
		loserID = b.Player1
	}

	tx, err := DB.Begin()
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	// Fetch current ratings (lock rows for update)
	var winnerRating, loserRating int
	err = tx.QueryRow(`SELECT rating FROM users WHERE id = $1 FOR UPDATE`, winnerID).Scan(&winnerRating)
	if err != nil {
		return fmt.Errorf("get winner rating: %w", err)
	}
	err = tx.QueryRow(`SELECT rating FROM users WHERE id = $1 FOR UPDATE`, loserID).Scan(&loserRating)
	if err != nil {
		return fmt.Errorf("get loser rating: %w", err)
	}

	newWinnerRating, newLoserRating := calcELO(winnerRating, loserRating)

	// Insert battle (idempotent)
	_, err = tx.Exec(
		`INSERT INTO battles (id, player1_id, player2_id, status, round, p1_wins, p2_wins, winner_id)
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
		 ON CONFLICT (id) DO NOTHING`,
		b.ID, b.Player1, b.Player2, "complete", b.Round, b.P1Wins, b.P2Wins, winnerID,
	)
	if err != nil {
		return fmt.Errorf("insert battle: %w", err)
	}

	// Insert rounds
	for _, r := range b.Rounds {
		p1TeamJSON, _ := json.Marshal(r.P1Team)
		p2TeamJSON, _ := json.Marshal(r.P2Team)
		roundID := generateRoundID()

		_, err = tx.Exec(
			`INSERT INTO battle_rounds (id, battle_id, round, p1_team, p2_team, p1_score, p2_score, winner_id)
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
			 ON CONFLICT (id) DO NOTHING`,
			roundID, b.ID, r.Round, p1TeamJSON, p2TeamJSON,
			r.Result.P1Score, r.Result.P2Score, r.Result.Winner,
		)
		if err != nil {
			return fmt.Errorf("insert round %d: %w", r.Round, err)
		}
	}

	// Update stats
	if err := UpdateStats(tx, winnerID, 1, 0, newWinnerRating); err != nil {
		return fmt.Errorf("update winner stats: %w", err)
	}
	if err := UpdateStats(tx, loserID, 0, 1, newLoserRating); err != nil {
		return fmt.Errorf("update loser stats: %w", err)
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}

	log.Printf("Persisted battle %s: %s (%d→%d) beat %s (%d→%d)",
		b.ID, winnerID, winnerRating, newWinnerRating, loserID, loserRating, newLoserRating)

	return nil
}

// calcELO returns (newWinnerRating, newLoserRating) using standard ELO with K=32.
func calcELO(winnerRating, loserRating int) (int, int) {
	k := 32.0
	expectedWinner := 1.0 / (1.0 + math.Pow(10, float64(loserRating-winnerRating)/400.0))
	expectedLoser := 1.0 - expectedWinner

	newWinner := winnerRating + int(math.Round(k*(1.0-expectedWinner)))
	newLoser := loserRating + int(math.Round(k*(0.0-expectedLoser)))
	if newLoser < 0 {
		newLoser = 0
	}

	return newWinner, newLoser
}

func generateRoundID() string {
	b := make([]byte, 8)
	rand.Read(b)
	return hex.EncodeToString(b)
}
