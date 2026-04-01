package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/matthewmyrick/catch-pokemon/api/internal/db"
	"github.com/matthewmyrick/catch-pokemon/api/internal/middleware"
	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
	"github.com/matthewmyrick/catch-pokemon/api/internal/verify"
)

type CreateTradeRequest struct {
	Offering   models.Pokemon      `json:"offering"`
	LookingFor string              `json:"looking_for"`
	PC         verify.SignedPayload `json:"pc_proof"`
}

type MakeOfferRequest struct {
	TradeID string              `json:"trade_id"`
	Pokemon models.Pokemon      `json:"pokemon"`
	PC      verify.SignedPayload `json:"pc_proof"`
}

type AcceptRejectRequest struct {
	TradeID string `json:"trade_id"`
	OfferID string `json:"offer_id"`
}

// ListTrades returns all open trades
func ListTrades(w http.ResponseWriter, r *http.Request) {
	trades, err := db.GetOpenTrades()
	if err != nil {
		http.Error(w, `{"error":"could not fetch trades"}`, http.StatusInternalServerError)
		return
	}
	if trades == nil {
		trades = []db.DBTrade{}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
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
	if req.LookingFor == "" {
		req.LookingFor = "open to offers"
	}

	// Verify PC if provided
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

	t, err := db.CreateTrade(userID, req.Offering, req.LookingFor)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusConflict)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
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

	t, err := db.GetTrade(tradeID)
	if err != nil {
		http.Error(w, `{"error":"trade not found"}`, http.StatusNotFound)
		return
	}

	offers, _ := db.GetOffers(tradeID)
	if offers == nil {
		offers = []db.DBTradeOffer{}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"trade":  t,
		"offers": offers,
	})
}

// MakeTradeOffer submits an offer
func MakeTradeOffer(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req MakeOfferRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	// Verify PC if provided
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

	offer, err := db.MakeOffer(req.TradeID, userID, req.Pokemon)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": err.Error()})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":  "offered",
		"offer":   offer,
		"message": "Your offer has been submitted!",
	})
}

// AcceptTradeOffer accepts an offer
func AcceptTradeOffer(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req AcceptRejectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if err := db.AcceptOffer(req.TradeID, req.OfferID, userID); err != nil {
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

// RejectTradeOffer rejects an offer
func RejectTradeOffer(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req AcceptRejectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if err := db.RejectOffer(req.TradeID, req.OfferID, userID); err != nil {
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

// CancelTrade cancels a trade
func CancelTrade(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	var req struct {
		TradeID string `json:"trade_id"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
		return
	}

	if err := db.CancelTrade(req.TradeID, userID); err != nil {
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

// MyTrade returns the user's active trade
func MyTrade(w http.ResponseWriter, r *http.Request) {
	userID := middleware.GetUserID(r)

	t, err := db.GetUserTrade(userID)
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"status":  "none",
			"message": "No active trade listing.",
		})
		return
	}

	offers, _ := db.GetOffers(t.ID)
	if offers == nil {
		offers = []db.DBTradeOffer{}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"trade":  t,
		"offers": offers,
	})
}
