package trade

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"log"
	"sync"
	"time"

	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

type Store struct {
	mu     sync.RWMutex
	trades map[string]*models.Trade
	offers map[string][]*models.TradeOffer // trade_id -> offers
	// Track one active listing per user
	userTrade map[string]string // user_id -> trade_id
}

func NewStore() *Store {
	s := &Store{
		trades:    make(map[string]*models.Trade),
		offers:    make(map[string][]*models.TradeOffer),
		userTrade: make(map[string]string),
	}
	go s.cleanup()
	return s
}

func (s *Store) CreateTrade(posterID string, offering models.Pokemon, lookingFor string) (*models.Trade, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Check if user already has an active listing
	if existingID, ok := s.userTrade[posterID]; ok {
		if t, exists := s.trades[existingID]; exists && t.Status == "open" {
			return nil, fmt.Errorf("you already have an active trade listing")
		}
	}

	id := generateID()
	now := time.Now()
	trade := &models.Trade{
		ID:         id,
		PosterID:   posterID,
		Offering:   offering,
		LookingFor: lookingFor,
		Status:     "open",
		CreatedAt:  now,
		ExpiresAt:  now.Add(72 * time.Hour), // 3 days
	}

	s.trades[id] = trade
	s.userTrade[posterID] = id
	log.Printf("Trade %s created by %s: offering %s, looking for %s", id, posterID, offering.Name, lookingFor)

	return trade, nil
}

func (s *Store) MakeOffer(tradeID, offerByID string, pokemon models.Pokemon) (*models.TradeOffer, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	trade, ok := s.trades[tradeID]
	if !ok {
		return nil, fmt.Errorf("trade not found")
	}
	if trade.Status != "open" {
		return nil, fmt.Errorf("trade is no longer open")
	}
	if trade.PosterID == offerByID {
		return nil, fmt.Errorf("you can't offer on your own trade")
	}

	// Check if user already made an offer on this trade
	for _, o := range s.offers[tradeID] {
		if o.OfferByID == offerByID && o.Status == "pending" {
			return nil, fmt.Errorf("you already have a pending offer on this trade")
		}
	}

	offer := &models.TradeOffer{
		ID:        generateID(),
		TradeID:   tradeID,
		OfferByID: offerByID,
		Pokemon:   pokemon,
		Status:    "pending",
		CreatedAt: time.Now(),
	}

	s.offers[tradeID] = append(s.offers[tradeID], offer)
	log.Printf("Offer %s on trade %s: %s offers %s", offer.ID, tradeID, offerByID, pokemon.Name)

	return offer, nil
}

func (s *Store) AcceptOffer(tradeID, offerID, userID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	trade, ok := s.trades[tradeID]
	if !ok {
		return fmt.Errorf("trade not found")
	}
	if trade.PosterID != userID {
		return fmt.Errorf("only the trade poster can accept offers")
	}
	if trade.Status != "open" {
		return fmt.Errorf("trade is no longer open")
	}

	// Find and accept the offer
	var accepted *models.TradeOffer
	for _, o := range s.offers[tradeID] {
		if o.ID == offerID {
			accepted = o
			break
		}
	}
	if accepted == nil {
		return fmt.Errorf("offer not found")
	}
	if accepted.Status != "pending" {
		return fmt.Errorf("offer is no longer pending")
	}

	// Accept this offer, reject all others
	accepted.Status = "accepted"
	for _, o := range s.offers[tradeID] {
		if o.ID != offerID && o.Status == "pending" {
			o.Status = "rejected"
		}
	}

	trade.Status = "accepted"
	delete(s.userTrade, trade.PosterID)

	log.Printf("Trade %s accepted: %s trades %s for %s's %s",
		tradeID, trade.PosterID, trade.Offering.Name, accepted.OfferByID, accepted.Pokemon.Name)

	return nil
}

func (s *Store) RejectOffer(tradeID, offerID, userID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	trade, ok := s.trades[tradeID]
	if !ok {
		return fmt.Errorf("trade not found")
	}
	if trade.PosterID != userID {
		return fmt.Errorf("only the trade poster can reject offers")
	}

	for _, o := range s.offers[tradeID] {
		if o.ID == offerID && o.Status == "pending" {
			o.Status = "rejected"
			log.Printf("Offer %s rejected on trade %s", offerID, tradeID)
			return nil
		}
	}

	return fmt.Errorf("offer not found or already handled")
}

func (s *Store) CancelTrade(tradeID, userID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	trade, ok := s.trades[tradeID]
	if !ok {
		return fmt.Errorf("trade not found")
	}
	if trade.PosterID != userID {
		return fmt.Errorf("only the trade poster can cancel")
	}
	if trade.Status != "open" {
		return fmt.Errorf("trade is not open")
	}

	trade.Status = "cancelled"
	// Reject all pending offers
	for _, o := range s.offers[tradeID] {
		if o.Status == "pending" {
			o.Status = "rejected"
		}
	}
	delete(s.userTrade, userID)

	log.Printf("Trade %s cancelled by %s", tradeID, userID)
	return nil
}

func (s *Store) GetOpenTrades() []*models.Trade {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var open []*models.Trade
	for _, t := range s.trades {
		if t.Status == "open" {
			open = append(open, t)
		}
	}
	return open
}

func (s *Store) GetTrade(tradeID string) *models.Trade {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.trades[tradeID]
}

func (s *Store) GetOffers(tradeID string) []*models.TradeOffer {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.offers[tradeID]
}

func (s *Store) GetUserTrade(userID string) *models.Trade {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if id, ok := s.userTrade[userID]; ok {
		return s.trades[id]
	}
	return nil
}

// cleanup expires old trades every minute
func (s *Store) cleanup() {
	ticker := time.NewTicker(60 * time.Second)
	defer ticker.Stop()

	for range ticker.C {
		s.mu.Lock()
		now := time.Now()
		for id, t := range s.trades {
			if t.Status == "open" && now.After(t.ExpiresAt) {
				t.Status = "expired"
				delete(s.userTrade, t.PosterID)
				// Reject pending offers
				for _, o := range s.offers[id] {
					if o.Status == "pending" {
						o.Status = "rejected"
					}
				}
				log.Printf("Trade %s expired", id)
			}
			// Clean up old completed trades after 1 hour
			if t.Status != "open" && now.Sub(t.CreatedAt) > time.Hour {
				delete(s.trades, id)
				delete(s.offers, id)
			}
		}
		s.mu.Unlock()
	}
}

func generateID() string {
	b := make([]byte, 8)
	rand.Read(b)
	return hex.EncodeToString(b)
}
