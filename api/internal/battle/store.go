package battle

import (
	"log"
	"sync"
	"time"

	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

// ActiveBattle tracks a battle in progress
type ActiveBattle struct {
	ID                string
	Player1           string
	Player2           string
	P1PC              []models.Pokemon
	P2PC              []models.Pokemon
	Status            string // selecting, battling, complete, abandoned
	Round             int
	P1Wins            int
	P2Wins            int
	P1Team            []models.Pokemon
	P2Team            []models.Pokemon
	P1Ready           bool
	P2Ready           bool
	Rounds            []RoundResult
	CreatedAt         time.Time
	UpdatedAt         time.Time
	SelectionDeadline time.Time // when the current selection phase expires
	P1LastSeen        time.Time // last heartbeat from player 1
	P2LastSeen        time.Time // last heartbeat from player 2
}

// RoundResult stores the outcome of one round
type RoundResult struct {
	Round  int              `json:"round"`
	P1Team []models.Pokemon `json:"p1_team"`
	P2Team []models.Pokemon `json:"p2_team"`
	Result Result           `json:"result"`
}

// Store holds all active battles in memory
type Store struct {
	mu         sync.RWMutex
	battles    map[string]*ActiveBattle
	userBattle map[string]string
}

func NewStore() *Store {
	s := &Store{
		battles:    make(map[string]*ActiveBattle),
		userBattle: make(map[string]string),
	}
	go s.cleanup()
	return s
}

func (s *Store) Create(b *ActiveBattle) {
	s.mu.Lock()
	defer s.mu.Unlock()
	b.CreatedAt = time.Now()
	b.UpdatedAt = time.Now()
	s.battles[b.ID] = b
	s.userBattle[b.Player1] = b.ID
	s.userBattle[b.Player2] = b.ID
}

func (s *Store) Get(battleID string) *ActiveBattle {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.battles[battleID]
}

func (s *Store) GetByUser(userID string) *ActiveBattle {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if battleID, ok := s.userBattle[userID]; ok {
		return s.battles[battleID]
	}
	return nil
}

func (s *Store) Update(b *ActiveBattle) {
	s.mu.Lock()
	defer s.mu.Unlock()
	b.UpdatedAt = time.Now()
	s.battles[b.ID] = b
}

func (s *Store) Remove(battleID string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if b, ok := s.battles[battleID]; ok {
		delete(s.userBattle, b.Player1)
		delete(s.userBattle, b.Player2)
		delete(s.battles, battleID)
	}
}

// cleanup removes stale battles every 60 seconds
func (s *Store) cleanup() {
	ticker := time.NewTicker(60 * time.Second)
	defer ticker.Stop()

	for range ticker.C {
		s.mu.Lock()
		now := time.Now()
		var toRemove []string

		for id, b := range s.battles {
			// Abandon battles stuck in selecting for > 15 minutes
			if b.Status == "selecting" && now.Sub(b.UpdatedAt) > 15*time.Minute {
				log.Printf("Abandoning stale battle %s (stuck in selecting for %s)", id, now.Sub(b.UpdatedAt))
				b.Status = "abandoned"
				toRemove = append(toRemove, id)
			}

			// Clean up completed battles after 5 minutes
			if b.Status == "complete" && now.Sub(b.UpdatedAt) > 5*time.Minute {
				toRemove = append(toRemove, id)
			}

			// Clean up abandoned battles after 1 minute
			if b.Status == "abandoned" && now.Sub(b.UpdatedAt) > 1*time.Minute {
				toRemove = append(toRemove, id)
			}
		}

		for _, id := range toRemove {
			if b, ok := s.battles[id]; ok {
				delete(s.userBattle, b.Player1)
				delete(s.userBattle, b.Player2)
				delete(s.battles, id)
				log.Printf("Cleaned up battle %s", id)
			}
		}
		s.mu.Unlock()
	}
}
