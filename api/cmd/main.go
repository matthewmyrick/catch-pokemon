package main

import (
	"log"
	"net/http"
	"os"

	"github.com/matthewmyrick/catch-pokemon/api/internal/db"
	"github.com/matthewmyrick/catch-pokemon/api/internal/handlers"
	"github.com/matthewmyrick/catch-pokemon/api/internal/matchmaking"
	"github.com/matthewmyrick/catch-pokemon/api/internal/middleware"
)

func main() {
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	// Connect to PostgreSQL (optional)
	if os.Getenv("DATABASE_URL") != "" {
		if err := db.Connect(); err != nil {
			log.Fatalf("Database connection failed: %v", err)
		}
		defer db.DB.Close()
	} else {
		log.Println("WARN: DATABASE_URL not set, running without database")
	}

	// Initialize matchmaking queue
	queue := matchmaking.NewQueue()
	go queue.Run()

	mux := http.NewServeMux()

	// Health check
	mux.HandleFunc("GET /health", handlers.Health)

	// Protected routes (authenticated via GitHub token)
	mux.Handle("GET /api/me", middleware.Auth(http.HandlerFunc(handlers.GetMe)))
	mux.Handle("POST /api/battle/join", middleware.Auth(handlers.BattleJoin(queue)))
	mux.Handle("POST /api/battle/select", middleware.Auth(http.HandlerFunc(handlers.BattleSelect)))
	mux.Handle("GET /api/battle/status", middleware.Auth(http.HandlerFunc(handlers.BattleStatus)))
	mux.Handle("GET /api/rankings", middleware.Auth(http.HandlerFunc(handlers.GetRankings)))
	mux.Handle("GET /api/ws", middleware.Auth(handlers.WebSocketHandler(queue)))

	// Trade routes (bulletin board)
	mux.Handle("GET /api/trades", middleware.Auth(http.HandlerFunc(handlers.ListTrades)))
	mux.Handle("GET /api/trade", middleware.Auth(http.HandlerFunc(handlers.GetTradeDetail)))
	mux.Handle("GET /api/trade/mine", middleware.Auth(http.HandlerFunc(handlers.MyTrade)))
	mux.Handle("POST /api/trade/create", middleware.Auth(http.HandlerFunc(handlers.CreateTrade)))
	mux.Handle("POST /api/trade/offer", middleware.Auth(http.HandlerFunc(handlers.MakeTradeOffer)))
	mux.Handle("POST /api/trade/accept", middleware.Auth(http.HandlerFunc(handlers.AcceptTradeOffer)))
	mux.Handle("POST /api/trade/reject", middleware.Auth(http.HandlerFunc(handlers.RejectTradeOffer)))
	mux.Handle("POST /api/trade/cancel", middleware.Auth(http.HandlerFunc(handlers.CancelTrade)))

	log.Printf("catch-pokemon API server starting on :%s", port)
	if err := http.ListenAndServe(":"+port, mux); err != nil {
		log.Fatal(err)
	}
}
