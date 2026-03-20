package middleware

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"
)

type contextKey string

const UserIDKey contextKey = "user_id"
const UserNameKey contextKey = "user_name"

// GitHubUser represents the response from GitHub's /user endpoint
type GitHubUser struct {
	Login     string `json:"login"`
	ID        int    `json:"id"`
	AvatarURL string `json:"avatar_url"`
}

// Token cache to avoid hitting GitHub API on every request
var (
	tokenCache     = make(map[string]*cachedUser)
	tokenCacheMu   sync.RWMutex
)

type cachedUser struct {
	user      *GitHubUser
	expiresAt time.Time
}

// Auth middleware validates GitHub tokens by calling the GitHub API
func Auth(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		authHeader := r.Header.Get("Authorization")
		if authHeader == "" {
			http.Error(w, `{"error":"missing authorization header. Run: gh auth token"}`, http.StatusUnauthorized)
			return
		}

		parts := strings.SplitN(authHeader, " ", 2)
		if len(parts) != 2 || parts[0] != "Bearer" {
			http.Error(w, `{"error":"invalid authorization format. Use: Bearer <token>"}`, http.StatusUnauthorized)
			return
		}

		token := parts[1]

		// Check cache first
		user := getCachedUser(token)
		if user == nil {
			// Verify token against GitHub API
			var err error
			user, err = verifyGitHubToken(token)
			if err != nil {
				http.Error(w, fmt.Sprintf(`{"error":"GitHub authentication failed: %s"}`, err.Error()), http.StatusUnauthorized)
				return
			}
			// Cache for 10 minutes
			cacheUser(token, user)
		}

		ctx := context.WithValue(r.Context(), UserIDKey, user.Login)
		ctx = context.WithValue(ctx, UserNameKey, user.Login)
		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

func verifyGitHubToken(token string) (*GitHubUser, error) {
	req, err := http.NewRequest("GET", "https://api.github.com/user", nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request")
	}

	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "catch-pokemon-api")

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("could not reach GitHub API")
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("invalid token (status %d: %s)", resp.StatusCode, string(body))
	}

	var user GitHubUser
	if err := json.NewDecoder(resp.Body).Decode(&user); err != nil {
		return nil, fmt.Errorf("failed to parse GitHub response")
	}

	if user.Login == "" {
		return nil, fmt.Errorf("could not determine GitHub username")
	}

	return &user, nil
}

func getCachedUser(token string) *GitHubUser {
	tokenCacheMu.RLock()
	defer tokenCacheMu.RUnlock()

	if cached, ok := tokenCache[token]; ok {
		if time.Now().Before(cached.expiresAt) {
			return cached.user
		}
	}
	return nil
}

func cacheUser(token string, user *GitHubUser) {
	tokenCacheMu.Lock()
	defer tokenCacheMu.Unlock()

	tokenCache[token] = &cachedUser{
		user:      user,
		expiresAt: time.Now().Add(10 * time.Minute),
	}
}

// GetUserID extracts the user ID from the request context
func GetUserID(r *http.Request) string {
	if id, ok := r.Context().Value(UserIDKey).(string); ok {
		return id
	}
	return ""
}
