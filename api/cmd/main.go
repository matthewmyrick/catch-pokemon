package main

import (
	"log"
	"net/http"
	"os"

	"github.com/matthewmyrick/catch-pokemon/api/internal/handlers"
	"github.com/matthewmyrick/catch-pokemon/api/internal/matchmaking"
	"github.com/matthewmyrick/catch-pokemon/api/internal/middleware"
)

func main() {
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	// Initialize matchmaking queue
	queue := matchmaking.NewQueue()
	go queue.Run()

	mux := http.NewServeMux()

	// Health check
	mux.HandleFunc("GET /health", handlers.Health)

	// Auth routes
	mux.HandleFunc("GET /auth/google", handlers.GoogleLogin)
	mux.HandleFunc("GET /auth/google/callback", handlers.GoogleCallback)

	// Protected routes
	mux.Handle("GET /api/me", middleware.Auth(http.HandlerFunc(handlers.GetMe)))
	mux.Handle("POST /api/battle/join", middleware.Auth(handlers.BattleJoin(queue)))
	mux.Handle("POST /api/battle/select", middleware.Auth(http.HandlerFunc(handlers.BattleSelect)))
	mux.Handle("GET /api/battle/status", middleware.Auth(http.HandlerFunc(handlers.BattleStatus)))
	mux.Handle("GET /api/rankings", middleware.Auth(http.HandlerFunc(handlers.GetRankings)))
	mux.Handle("GET /api/ws", middleware.Auth(handlers.WebSocketHandler(queue)))

	log.Printf("catch-pokemon API server starting on :%s", port)
	if err := http.ListenAndServe(":"+port, mux); err != nil {
		log.Fatal(err)
	}
}
