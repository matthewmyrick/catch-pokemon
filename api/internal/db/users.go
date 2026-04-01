package db

import (
	"database/sql"
	"fmt"

	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

// UpsertUser creates the user if they don't exist. No-ops on conflict.
func UpsertUser(id, name string) error {
	email := id + "@github"
	_, err := DB.Exec(
		`INSERT INTO users (id, email, name) VALUES ($1, $2, $3)
		 ON CONFLICT (id) DO NOTHING`,
		id, email, name,
	)
	return err
}

func GetUser(id string) (*models.User, error) {
	u := &models.User{}
	err := DB.QueryRow(
		`SELECT id, email, name, wins, losses, rating, created_at
		 FROM users WHERE id = $1`, id,
	).Scan(&u.ID, &u.Email, &u.Name, &u.Wins, &u.Losses, &u.Rating, &u.CreatedAt)
	if err != nil {
		return nil, err
	}
	return u, nil
}

func GetRankings(limit, offset int) ([]models.User, error) {
	rows, err := DB.Query(
		`SELECT id, name, wins, losses, rating
		 FROM users WHERE wins > 0 OR losses > 0
		 ORDER BY rating DESC, wins DESC
		 LIMIT $1 OFFSET $2`, limit, offset,
	)
	if err != nil {
		return nil, fmt.Errorf("query rankings: %w", err)
	}
	defer rows.Close()

	var users []models.User
	for rows.Next() {
		var u models.User
		if err := rows.Scan(&u.ID, &u.Name, &u.Wins, &u.Losses, &u.Rating); err != nil {
			return nil, fmt.Errorf("scan ranking: %w", err)
		}
		users = append(users, u)
	}
	return users, rows.Err()
}

func UpdateStats(tx *sql.Tx, userID string, addWins, addLosses, newRating int) error {
	_, err := tx.Exec(
		`UPDATE users SET wins = wins + $2, losses = losses + $3, rating = $4
		 WHERE id = $1`,
		userID, addWins, addLosses, newRating,
	)
	return err
}
