package db

import (
	"database/sql"
	"fmt"
	"log"
	"os"

	_ "github.com/lib/pq"
)

var DB *sql.DB

func Connect() error {
	dsn := os.Getenv("DATABASE_URL")
	if dsn == "" {
		return fmt.Errorf("DATABASE_URL not set")
	}

	var err error
	DB, err = sql.Open("postgres", dsn)
	if err != nil {
		return fmt.Errorf("sql.Open: %w", err)
	}

	DB.SetMaxOpenConns(10)
	DB.SetMaxIdleConns(5)

	if err := DB.Ping(); err != nil {
		return fmt.Errorf("db ping: %w", err)
	}

	log.Println("Connected to PostgreSQL")

	if err := runMigrations(); err != nil {
		return fmt.Errorf("migrations: %w", err)
	}

	return nil
}

func runMigrations() error {
	migrations := []string{
		// 001_init.sql
		`CREATE TABLE IF NOT EXISTS users (
			id TEXT PRIMARY KEY,
			email TEXT UNIQUE NOT NULL,
			name TEXT NOT NULL DEFAULT '',
			wins INTEGER NOT NULL DEFAULT 0,
			losses INTEGER NOT NULL DEFAULT 0,
			rating INTEGER NOT NULL DEFAULT 1000,
			created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
		)`,
		`CREATE TABLE IF NOT EXISTS battles (
			id TEXT PRIMARY KEY,
			player1_id TEXT NOT NULL REFERENCES users(id),
			player2_id TEXT NOT NULL REFERENCES users(id),
			status TEXT NOT NULL DEFAULT 'selecting',
			round INTEGER NOT NULL DEFAULT 1,
			p1_wins INTEGER NOT NULL DEFAULT 0,
			p2_wins INTEGER NOT NULL DEFAULT 0,
			winner_id TEXT REFERENCES users(id),
			created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
		)`,
		`CREATE TABLE IF NOT EXISTS battle_rounds (
			id TEXT PRIMARY KEY,
			battle_id TEXT NOT NULL REFERENCES battles(id),
			round INTEGER NOT NULL,
			p1_team JSONB NOT NULL DEFAULT '[]',
			p2_team JSONB NOT NULL DEFAULT '[]',
			p1_score DOUBLE PRECISION,
			p2_score DOUBLE PRECISION,
			winner_id TEXT REFERENCES users(id),
			created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
		)`,
		`CREATE INDEX IF NOT EXISTS idx_battles_player1 ON battles(player1_id)`,
		`CREATE INDEX IF NOT EXISTS idx_battles_player2 ON battles(player2_id)`,
		`CREATE INDEX IF NOT EXISTS idx_battles_status ON battles(status)`,
		`CREATE INDEX IF NOT EXISTS idx_battle_rounds_battle ON battle_rounds(battle_id)`,
		`CREATE INDEX IF NOT EXISTS idx_users_rating ON users(rating DESC)`,
		// 002_trades.sql
		`CREATE TABLE IF NOT EXISTS trades (
			id TEXT PRIMARY KEY,
			poster_id TEXT NOT NULL,
			offering_name TEXT NOT NULL,
			offering_types JSONB NOT NULL DEFAULT '[]',
			offering_power INTEGER NOT NULL DEFAULT 0,
			offering_shiny BOOLEAN NOT NULL DEFAULT false,
			looking_for TEXT NOT NULL,
			status TEXT NOT NULL DEFAULT 'open',
			created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
			expires_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() + INTERVAL '3 days'
		)`,
		`CREATE TABLE IF NOT EXISTS trade_offers (
			id TEXT PRIMARY KEY,
			trade_id TEXT NOT NULL REFERENCES trades(id),
			offer_by_id TEXT NOT NULL,
			pokemon_name TEXT NOT NULL,
			pokemon_types JSONB NOT NULL DEFAULT '[]',
			pokemon_power INTEGER NOT NULL DEFAULT 0,
			pokemon_shiny BOOLEAN NOT NULL DEFAULT false,
			status TEXT NOT NULL DEFAULT 'pending',
			created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
		)`,
		`CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status)`,
		`CREATE INDEX IF NOT EXISTS idx_trades_poster ON trades(poster_id)`,
		`CREATE INDEX IF NOT EXISTS idx_trades_expires ON trades(expires_at)`,
		`CREATE INDEX IF NOT EXISTS idx_trade_offers_trade ON trade_offers(trade_id)`,
	}

	for i, m := range migrations {
		if _, err := DB.Exec(m); err != nil {
			return fmt.Errorf("migration %d: %w", i+1, err)
		}
	}

	log.Println("Migrations applied")
	return nil
}
