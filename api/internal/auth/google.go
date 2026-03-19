package auth

import (
	"os"
)

// Config holds Google OAuth configuration
type Config struct {
	ClientID     string
	ClientSecret string
	RedirectURL  string
}

// NewConfig creates OAuth config from environment variables
func NewConfig() *Config {
	return &Config{
		ClientID:     os.Getenv("GOOGLE_CLIENT_ID"),
		ClientSecret: os.Getenv("GOOGLE_CLIENT_SECRET"),
		RedirectURL:  os.Getenv("GOOGLE_REDIRECT_URL"),
	}
}
