package battle

import (
	"math/rand"

	"github.com/matthewmyrick/catch-pokemon/api/internal/models"
)

// Result holds the outcome of a single round
type Result struct {
	Winner       string  `json:"winner"`        // user ID of winner
	P1Score      float64 `json:"p1_score"`
	P2Score      float64 `json:"p2_score"`
	P1TypeBonus  float64 `json:"p1_type_bonus"`
	P2TypeBonus  float64 `json:"p2_type_bonus"`
	P1PowerTotal int     `json:"p1_power_total"`
	P2PowerTotal int     `json:"p2_power_total"`
	RNGRoll      float64 `json:"rng_roll"`
}

// CalculateTypeAdvantage returns the type multiplier for team1 attacking team2
func CalculateTypeAdvantage(team1, team2 []models.Pokemon) float64 {
	totalMultiplier := 0.0
	matchups := 0

	for _, attacker := range team1 {
		for _, defender := range team2 {
			for _, atkType := range attacker.Types {
				for _, defType := range defender.Types {
					if chart, ok := models.TypeChart[atkType]; ok {
						if mult, ok := chart[defType]; ok {
							totalMultiplier += mult
							matchups++
							continue
						}
					}
					totalMultiplier += 1.0
					matchups++
				}
			}
		}
	}

	if matchups == 0 {
		return 1.0
	}
	return totalMultiplier / float64(matchups)
}

// TeamPower returns the sum of power rankings for a team
func TeamPower(team []models.Pokemon) int {
	total := 0
	for _, p := range team {
		total += p.PowerRank
	}
	return total
}

// RunRound executes a battle round between two teams
// Formula: 40% power rank + 40% type advantage + 20% RNG
func RunRound(p1ID string, p1Team []models.Pokemon, p2ID string, p2Team []models.Pokemon) Result {
	p1Power := TeamPower(p1Team)
	p2Power := TeamPower(p2Team)

	p1TypeAdv := CalculateTypeAdvantage(p1Team, p2Team)
	p2TypeAdv := CalculateTypeAdvantage(p2Team, p1Team)

	// Normalize power to 0-1 range
	maxPower := float64(p1Power + p2Power)
	p1PowerNorm := float64(p1Power) / maxPower
	p2PowerNorm := float64(p2Power) / maxPower

	// Normalize type advantage to 0-1 range
	totalTypeAdv := p1TypeAdv + p2TypeAdv
	p1TypeNorm := p1TypeAdv / totalTypeAdv
	p2TypeNorm := p2TypeAdv / totalTypeAdv

	// RNG component
	rngRoll := rand.Float64()

	// Final scores: 40% power + 40% type + 20% RNG
	p1Score := (0.4 * p1PowerNorm) + (0.4 * p1TypeNorm) + (0.2 * rngRoll)
	p2Score := (0.4 * p2PowerNorm) + (0.4 * p2TypeNorm) + (0.2 * (1.0 - rngRoll))

	winner := p1ID
	if p2Score > p1Score {
		winner = p2ID
	}

	return Result{
		Winner:       winner,
		P1Score:      p1Score,
		P2Score:      p2Score,
		P1TypeBonus:  p1TypeAdv,
		P2TypeBonus:  p2TypeAdv,
		P1PowerTotal: p1Power,
		P2PowerTotal: p2Power,
		RNGRoll:      rngRoll,
	}
}
