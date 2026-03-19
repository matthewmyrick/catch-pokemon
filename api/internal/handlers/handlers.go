package handlers

import (
	"encoding/json"
	"log"
	"net/http"
	"time"

	"github.com/matthewmyrick/catch-pokemon/api/internal/battle"
	"github.com/matthewmyrick/catch-pokemon/api/internal/matchmaking"
	"github.com/matthewmyrick/catch-pokemon/api/internal/middleware"
	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

var Battles = battle.NewStore()

func Health(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
}

func GoogleLogin(w http.ResponseWriter, r *http.Request) {
	http.Error(w, `{"error":"not implemented"}`, http.StatusNotImplemented)
}

func GoogleCallback(w http.ResponseWriter, r *http.Request) {
	http.Error(w, `{"error":"not implemented"}`, http.StatusNotImplemented)
}

func GetMe(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"user_id": userID})
}

// BattleJoin is a long-poll endpoint — it blocks until a match is found (up to 60s)
func BattleJoin(queue *matchmaking.Queue) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		userID := middleware.GetUserID(r)

		var pc []models.Pokemon
		if err := json.NewDecoder(r.Body).Decode(&pc); err != nil {
			http.Error(w, `{"error":"invalid PC data"}`, http.StatusBadRequest)
			return
		}

		if len(pc) < 6 {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusBadRequest)
			json.NewEncoder(w).Encode(map[string]string{
				"error": "You need at least 6 Pokemon. Go out there and catch em all!",
			})
			return
		}

		cancel := make(chan struct{})
		player := &matchmaking.Player{
			UserID: userID,
			PC:     pc,
			Notify: make(chan *matchmaking.Match, 1),
			Cancel: cancel,
		}

		// Close cancel channel when the HTTP request ends (client disconnects)
		go func() {
			<-r.Context().Done()
			close(cancel)
		}()

		queue.Join(player)

		// Long-poll: wait up to 60 seconds for a match
		select {
		case match := <-player.Notify:
			// Determine opponent
			var opponentPC []models.Pokemon
			var opponentID string
			if match.Player1.UserID == userID {
				opponentPC = match.Player2.PC
				opponentID = match.Player2.UserID
			} else {
				opponentPC = match.Player1.PC
				opponentID = match.Player1.UserID
			}

			// Create battle in store (2 min selection deadline)
			now := time.Now()
			activeBattle := &battle.ActiveBattle{
				ID:                match.ID,
				Player1:           match.Player1.UserID,
				Player2:           match.Player2.UserID,
				P1PC:              match.Player1.PC,
				P2PC:              match.Player2.PC,
				Status:            "selecting",
				Round:             1,
				SelectionDeadline: now.Add(2 * time.Minute),
				P1LastSeen:        now,
				P2LastSeen:        now,
			}
			Battles.Create(activeBattle)

			log.Printf("Battle %s created: %s vs %s", match.ID, match.Player1.UserID, match.Player2.UserID)

			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]interface{}{
				"status":      "matched",
				"battle_id":   match.ID,
				"opponent_id": opponentID,
				"opponent_pc": opponentPC,
				"round":       1,
				"message":     "Opponent found! Select your team of 6 Pokemon.",
			})

		case <-time.After(60 * time.Second):
			queue.Leave(userID)
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]string{
				"status":  "timeout",
				"message": "No opponent found. Try again later.",
			})

		case <-r.Context().Done():
			queue.Leave(userID)
		}
	})
}

// SelectRequest is the body for team selection
type SelectRequest struct {
	BattleID string           `json:"battle_id"`
	Team     []models.Pokemon `json:"team"`
}

// BattleSelect handles team selection for a round
func BattleSelect(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req SelectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if len(req.Team) != 6 {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{
			"error": "You must select exactly 6 Pokemon for battle.",
		})
		return
	}

	b := Battles.Get(req.BattleID)
	if b == nil {
		http.Error(w, `{"error":"battle not found"}`, http.StatusNotFound)
		return
	}

	if b.Status != "selecting" {
		http.Error(w, `{"error":"battle is not in selection phase"}`, http.StatusBadRequest)
		return
	}

	// Record team selection
	if userID == b.Player1 {
		b.P1Team = req.Team
		b.P1Ready = true
	} else if userID == b.Player2 {
		b.P2Team = req.Team
		b.P2Ready = true
	} else {
		http.Error(w, `{"error":"you are not in this battle"}`, http.StatusForbidden)
		return
	}

	// If both players selected, run the round
	if b.P1Ready && b.P2Ready {
		b.Status = "battling"
		result := battle.RunRound(b.Player1, b.P1Team, b.Player2, b.P2Team)

		roundResult := battle.RoundResult{
			Round:  b.Round,
			P1Team: b.P1Team,
			P2Team: b.P2Team,
			Result: result,
		}
		b.Rounds = append(b.Rounds, roundResult)

		if result.Winner == b.Player1 {
			b.P1Wins++
		} else {
			b.P2Wins++
		}

		log.Printf("Battle %s Round %d: %s wins (P1: %.3f vs P2: %.3f)",
			b.ID, b.Round, result.Winner, result.P1Score, result.P2Score)

		// Check if battle is over (best of 5 = first to 3)
		if b.P1Wins >= 3 || b.P2Wins >= 3 {
			b.Status = "complete"
			winner := b.Player1
			if b.P2Wins > b.P1Wins {
				winner = b.Player2
			}
			log.Printf("Battle %s complete: %s wins %d-%d", b.ID, winner, b.P1Wins, b.P2Wins)
		} else {
			// Next round (reset 2 min deadline)
			b.Round++
			b.P1Team = nil
			b.P2Team = nil
			b.P1Ready = false
			b.P2Ready = false
			b.Status = "selecting"
			b.SelectionDeadline = time.Now().Add(2 * time.Minute)
		}

		Battles.Update(b)
	} else {
		Battles.Update(b)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":    "selected",
		"your_team": req.Team,
		"message":   "Team locked in. Waiting for opponent...",
	})
}

// BattleStatus returns the current state of a battle (also acts as heartbeat)
func BattleStatus(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	b := Battles.GetByUser(userID)
	if b == nil {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"status":  "none",
			"message": "No active battle.",
		})
		return
	}

	// Record heartbeat
	now := time.Now()
	if userID == b.Player1 {
		b.P1LastSeen = now
	} else if userID == b.Player2 {
		b.P2LastSeen = now
	}

	// Check if opponent disconnected (no heartbeat for 15 seconds)
	if b.Status == "selecting" {
		p1Gone := !b.P1LastSeen.IsZero() && now.Sub(b.P1LastSeen) > 15*time.Second
		p2Gone := !b.P2LastSeen.IsZero() && now.Sub(b.P2LastSeen) > 15*time.Second

		if p1Gone && userID == b.Player2 {
			b.Status = "complete"
			b.P2Wins = 3
			log.Printf("Battle %s: %s disconnected, %s wins by forfeit", b.ID, b.Player1, b.Player2)
			Battles.Update(b)
		} else if p2Gone && userID == b.Player1 {
			b.Status = "complete"
			b.P1Wins = 3
			log.Printf("Battle %s: %s disconnected, %s wins by forfeit", b.ID, b.Player2, b.Player1)
			Battles.Update(b)
		}
	}

	// Check if selection deadline has passed
	if b.Status == "selecting" && now.After(b.SelectionDeadline) {
		if !b.P1Ready && !b.P2Ready {
			// Neither selected — draw, battle abandoned
			b.Status = "abandoned"
			log.Printf("Battle %s abandoned: neither player selected in time", b.ID)
		} else if !b.P1Ready {
			// Player 1 didn't select — player 2 wins the round
			b.P2Wins++
			log.Printf("Battle %s Round %d: %s forfeited (timeout)", b.ID, b.Round, b.Player1)
		} else if !b.P2Ready {
			// Player 2 didn't select — player 1 wins the round
			b.P1Wins++
			log.Printf("Battle %s Round %d: %s forfeited (timeout)", b.ID, b.Round, b.Player2)
		}

		// Check if battle is over after forfeit
		if b.P1Wins >= 3 || b.P2Wins >= 3 {
			b.Status = "complete"
			winner := b.Player1
			if b.P2Wins > b.P1Wins {
				winner = b.Player2
			}
			log.Printf("Battle %s complete (forfeit): %s wins %d-%d", b.ID, winner, b.P1Wins, b.P2Wins)
		} else if b.Status != "abandoned" {
			// Next round
			b.Round++
			b.P1Team = nil
			b.P2Team = nil
			b.P1Ready = false
			b.P2Ready = false
			b.Status = "selecting"
			b.SelectionDeadline = time.Now().Add(2 * time.Minute)
		}

		Battles.Update(b)
	}

	// Build response
	response := map[string]interface{}{
		"battle_id": b.ID,
		"status":    b.Status,
		"round":     b.Round,
		"p1_wins":   b.P1Wins,
		"p2_wins":   b.P2Wins,
		"rounds":    b.Rounds,
	}

	// Show who you are
	if userID == b.Player1 {
		response["you"] = b.Player1
		response["opponent"] = b.Player2
		response["your_ready"] = b.P1Ready
		response["opponent_ready"] = b.P2Ready
	} else {
		response["you"] = b.Player2
		response["opponent"] = b.Player1
		response["your_ready"] = b.P2Ready
		response["opponent_ready"] = b.P1Ready
	}

	if b.Status == "complete" {
		winner := b.Player1
		if b.P2Wins > b.P1Wins {
			winner = b.Player2
		}
		response["winner"] = winner
		if winner == userID {
			response["message"] = "You won the battle!"
		} else {
			response["message"] = "You lost the battle."
		}
	}

	if b.Status == "abandoned" {
		response["message"] = "Battle abandoned. Neither player selected in time."
	}

	// Show remaining selection time
	if b.Status == "selecting" {
		remaining := time.Until(b.SelectionDeadline).Seconds()
		if remaining < 0 {
			remaining = 0
		}
		response["selection_remaining_seconds"] = int(remaining)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func GetRankings(w http.ResponseWriter, r *http.Request) {
	http.Error(w, `{"error":"not implemented"}`, http.StatusNotImplemented)
}

func WebSocketHandler(queue *matchmaking.Queue) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, `{"error":"not implemented"}`, http.StatusNotImplemented)
	})
}
