package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/matthewmyrick/catch-pokemon/api/internal/middleware"
	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
	"github.com/matthewmyrick/catch-pokemon/api/internal/trade"
	"github.com/matthewmyrick/catch-pokemon/api/internal/verify"
)

var Trades = trade.NewStore()

type CreateTradeRequest struct {
	Offering   models.Pokemon       `json:"offering"`
	LookingFor string               `json:"looking_for"`
	PC         verify.SignedPayload  `json:"pc_proof"`
}

type MakeOfferRequest struct {
	TradeID string                  `json:"trade_id"`
	Pokemon models.Pokemon          `json:"pokemon"`
	PC      verify.SignedPayload    `json:"pc_proof"`
}

type AcceptRejectRequest struct {
	TradeID string `json:"trade_id"`
	OfferID string `json:"offer_id"`
}

// ListTrades returns all open trades on the bulletin board
func ListTrades(w http.ResponseWriter, r *http.Request) {
	trades := Trades.GetOpenTrades()
	if trades == nil {
		trades = []*models.Trade{}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{
		"trades": trades,
		"count":  len(trades),
	})
}

// CreateTrade posts a new trade listing
func CreateTrade(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req CreateTradeRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if req.Offering.Name == "" {
		http.Error(w, `{"error":"must specify a Pokemon to offer"}`, http.StatusBadRequest)
		return
	}
	// looking_for is optional — just posting a pokemon for offers
	if req.LookingFor == "" {
		req.LookingFor = "open to offers"
	}

	// Verify the signed PC payload if provided
	if req.PC.Signature != "" {
		if err := verify.VerifyPayload(&req.PC); err != nil {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusForbidden)
			json.NewEncoder(w).Encode(map[string]string{"error": "PC verification failed: " + err.Error()})
			return
		}
		if !verify.HasPokemon(req.PC.PC, req.Offering.Name) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusForbidden)
			json.NewEncoder(w).Encode(map[string]string{"error": "You don't have " + req.Offering.Name + " in your PC"})
			return
		}
	}

	t, err := Trades.CreateTrade(userID, req.Offering, req.LookingFor)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusConflict)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{
		"status":  "posted",
		"trade":   t,
		"message": "Trade posted to the bulletin board!",
	})
}

// GetTradeDetail returns a trade with its offers
func GetTradeDetail(w http.ResponseWriter, r *http.Request) {
	tradeID := r.URL.Query().Get("id")
	if tradeID == "" {
		http.Error(w, `{"error":"missing trade id"}`, http.StatusBadRequest)
		return
	}

	t := Trades.GetTrade(tradeID)
	if t == nil {
		http.Error(w, `{"error":"trade not found"}`, http.StatusNotFound)
		return
	}

	offers := Trades.GetOffers(tradeID)
	if offers == nil {
		offers = []*models.TradeOffer{}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{
		"trade":  t,
		"offers": offers,
	})
}

// MakeTradeOffer submits an offer for a trade listing
func MakeTradeOffer(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req MakeOfferRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	// Verify the signed PC payload if provided
	if req.PC.Signature != "" {
		if err := verify.VerifyPayload(&req.PC); err != nil {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusForbidden)
			json.NewEncoder(w).Encode(map[string]string{"error": "PC verification failed: " + err.Error()})
			return
		}
		if !verify.HasPokemon(req.PC.PC, req.Pokemon.Name) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusForbidden)
			json.NewEncoder(w).Encode(map[string]string{"error": "You don't have " + req.Pokemon.Name + " in your PC"})
			return
		}
	}

	offer, err := Trades.MakeOffer(req.TradeID, userID, req.Pokemon)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{
		"status":  "offered",
		"offer":   offer,
		"message": "Your offer has been submitted!",
	})
}

// AcceptTradeOffer accepts an offer on your trade listing
func AcceptTradeOffer(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req AcceptRejectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if err := Trades.AcceptOffer(req.TradeID, req.OfferID, userID); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"status":  "accepted",
		"message": "Trade accepted! Pokemon have been exchanged.",
	})
}

// RejectTradeOffer rejects an offer on your trade listing
func RejectTradeOffer(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req AcceptRejectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if err := Trades.RejectOffer(req.TradeID, req.OfferID, userID); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"status":  "rejected",
		"message": "Offer rejected.",
	})
}

// CancelTrade cancels your trade listing
func CancelTrade(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req struct {
		TradeID string `json:"trade_id"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if err := Trades.CancelTrade(req.TradeID, userID); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"status":  "cancelled",
		"message": "Trade listing cancelled.",
	})
}

// MyTrade returns the user's active trade listing
func MyTrade(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	t := Trades.GetUserTrade(userID)
	if t == nil {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"status":  "none",
			"message": "No active trade listing.",
		})
		return
	}

	offers := Trades.GetOffers(t.ID)
	if offers == nil {
		offers = []*models.TradeOffer{}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{
		"trade":  t,
		"offers": offers,
	})
}
