package verify

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"time"

	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

// Domain separation constants — must match the Rust CLI exactly
var (
	kdfDomain  = []byte("catch-pokemon:kdf:v1")
	signDomain = []byte("catch-pokemon:sign:v1")
)

// DeriveSigningKey derives the signing key from BUILD_SECRET_KEY
// Must produce the same key as the Rust CLI's derive_signing_key()
func DeriveSigningKey() ([]byte, error) {
	secretStr := os.Getenv("BUILD_SECRET_KEY")
	if secretStr == "" {
		return nil, fmt.Errorf("BUILD_SECRET_KEY not set")
	}

	// Hash the secret string the same way build.rs does
	secret := hashKey(secretStr)

	// Step 1: Domain-separated intermediate key
	mac := hmac.New(sha256.New, secret[:])
	mac.Write(kdfDomain)
	key := mac.Sum(nil)

	// Step 2: Key stretching — 10,000 HMAC rounds
	for round := uint32(0); round < 10_000; round++ {
		mac := hmac.New(sha256.New, key)
		// Write round as little-endian 4 bytes
		mac.Write([]byte{byte(round), byte(round >> 8), byte(round >> 16), byte(round >> 24)})
		mac.Write(signDomain)
		key = mac.Sum(nil)
	}

	return key, nil
}

// hashKey reproduces the Rust build.rs hash_key() function exactly
func hashKey(input string) [32]byte {
	bytes := []byte(input)
	var state [32]byte

	// Initialize with input bytes spread across state
	for i, b := range bytes {
		state[i%32] ^= b
		state[(i+13)%32] = state[(i+13)%32] + b // wrapping add
		state[(i+7)%32] = state[(i+7)%32] * (b | 1) // wrapping mul
	}

	// Mix thoroughly — 256 rounds
	for round := uint16(0); round < 256; round++ {
		for i := 0; i < 32; i++ {
			prev := state[(i+31)%32]
			next := state[(i+1)%32]
			state[i] = state[i] +
				rotateLeft(prev, 3) +
				rotateRight(next, 2) +
				byte(round) +
				byte(i)
		}
	}

	return state
}

func rotateLeft(b byte, n uint) byte {
	return (b << n) | (b >> (8 - n))
}

func rotateRight(b byte, n uint) byte {
	return (b >> n) | (b << (8 - n))
}

// SignedPayload is what the CLI sends to the API
type SignedPayload struct {
	PC        []models.CaughtPokemon `json:"pc"`
	Timestamp int64                  `json:"timestamp"`
	Signature string                 `json:"signature"`
}

// CaughtPokemon matches the Rust CaughtPokemon struct for canonical serialization
// (defined in models for reuse)

// VerifyPayload checks that the signed PC payload is legitimate
func VerifyPayload(payload *SignedPayload) error {
	key, err := DeriveSigningKey()
	if err != nil {
		return fmt.Errorf("server key error: %w", err)
	}

	// Check timestamp is not too old (30 minutes)
	now := time.Now().Unix()
	if abs(now-payload.Timestamp) > 1800 {
		return fmt.Errorf("payload expired")
	}

	// Rebuild the canonical data string the same way the CLI does
	// Format: json of pc array + timestamp
	canonicalData := fmt.Sprintf("%d:", payload.Timestamp)
	for _, p := range payload.PC {
		canonicalData += fmt.Sprintf("%s|%s|%s|%v,", p.Name, p.CaughtAt, p.BallUsed, p.Shiny)
	}

	mac := hmac.New(sha256.New, key)
	mac.Write([]byte(canonicalData))
	expectedSig := hex.EncodeToString(mac.Sum(nil))

	if payload.Signature != expectedSig {
		return fmt.Errorf("invalid signature")
	}

	return nil
}

// HasPokemon checks if a verified PC contains a specific Pokemon
func HasPokemon(pc []models.CaughtPokemon, name string) bool {
	for _, p := range pc {
		if p.Name == name {
			return true
		}
	}
	return false
}

// CountPokemon returns how many of a specific Pokemon are in the PC
func CountPokemon(pc []models.CaughtPokemon, name string) int {
	count := 0
	for _, p := range pc {
		if p.Name == name {
			count++
		}
	}
	return count
}

func abs(x int64) int64 {
	if x < 0 {
		return -x
	}
	return x
}
