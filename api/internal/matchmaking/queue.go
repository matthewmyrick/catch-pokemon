package matchmaking

import (
	"crypto/rand"
	"encoding/hex"
	"log"
	"sync"
	"time"

	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

// Player represents a user waiting in the matchmaking queue
type Player struct {
	UserID   string
	PC       []models.Pokemon
	Notify   chan *Match
	JoinedAt time.Time
	Cancel   chan struct{} // closed when player disconnects
}

// Match represents a matched pair of players
type Match struct {
	ID      string
	Player1 *Player
	Player2 *Player
}

// Queue manages the matchmaking queue
type Queue struct {
	mu      sync.Mutex
	waiting []*Player
	join    chan *Player
	leave   chan string
}

func NewQueue() *Queue {
	return &Queue{
		join:  make(chan *Player, 100),
		leave: make(chan string, 100),
	}
}

func (q *Queue) Join(player *Player) {
	player.JoinedAt = time.Now()
	q.join <- player
}

func (q *Queue) Leave(userID string) {
	q.leave <- userID
}

func (q *Queue) Run() {
	// Cleanup stale entries every 30 seconds
	cleanupTicker := time.NewTicker(30 * time.Second)
	defer cleanupTicker.Stop()

	for {
		select {
		case player := <-q.join:
			q.mu.Lock()
			// Remove any existing entry for this user (reconnect case)
			for i, p := range q.waiting {
				if p.UserID == player.UserID {
					q.waiting = append(q.waiting[:i], q.waiting[i+1:]...)
					break
				}
			}
			q.waiting = append(q.waiting, player)
			log.Printf("Player %s joined queue. Queue size: %d", player.UserID, len(q.waiting))

			q.tryMatch()
			q.mu.Unlock()

		case userID := <-q.leave:
			q.mu.Lock()
			for i, p := range q.waiting {
				if p.UserID == userID {
					q.waiting = append(q.waiting[:i], q.waiting[i+1:]...)
					log.Printf("Player %s left queue. Queue size: %d", userID, len(q.waiting))
					break
				}
			}
			q.mu.Unlock()

		case <-cleanupTicker.C:
			q.mu.Lock()
			now := time.Now()
			var alive []*Player
			for _, p := range q.waiting {
				// Remove players who have been waiting more than 90 seconds
				// (their long-poll would have timed out at 60s)
				if now.Sub(p.JoinedAt) > 90*time.Second {
					log.Printf("Removing stale player %s from queue (joined %s ago)", p.UserID, now.Sub(p.JoinedAt))
					continue
				}
				// Check if player's cancel channel is closed (disconnected)
				select {
				case <-p.Cancel:
					log.Printf("Removing disconnected player %s from queue", p.UserID)
					continue
				default:
				}
				alive = append(alive, p)
			}
			if len(alive) != len(q.waiting) {
				log.Printf("Queue cleanup: %d -> %d players", len(q.waiting), len(alive))
				q.waiting = alive
			}
			q.mu.Unlock()
		}
	}
}

func (q *Queue) tryMatch() {
	// Remove disconnected players before matching
	var alive []*Player
	for _, p := range q.waiting {
		select {
		case <-p.Cancel:
			log.Printf("Skipping disconnected player %s", p.UserID)
			continue
		default:
			alive = append(alive, p)
		}
	}
	q.waiting = alive

	if len(q.waiting) >= 2 {
		p1 := q.waiting[0]
		p2 := q.waiting[1]
		q.waiting = q.waiting[2:]

		match := &Match{
			ID:      generateMatchID(),
			Player1: p1,
			Player2: p2,
		}

		log.Printf("Match found: %s vs %s (match %s)", p1.UserID, p2.UserID, match.ID)

		p1.Notify <- match
		p2.Notify <- match
	}
}

func generateMatchID() string {
	b := make([]byte, 8)
	rand.Read(b)
	return hex.EncodeToString(b)
}
