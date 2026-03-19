CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    wins INTEGER NOT NULL DEFAULT 0,
    losses INTEGER NOT NULL DEFAULT 0,
    rating INTEGER NOT NULL DEFAULT 1000,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS battles (
    id TEXT PRIMARY KEY,
    player1_id TEXT NOT NULL REFERENCES users(id),
    player2_id TEXT NOT NULL REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'selecting', -- selecting, battling, complete
    round INTEGER NOT NULL DEFAULT 1,
    p1_wins INTEGER NOT NULL DEFAULT 0,
    p2_wins INTEGER NOT NULL DEFAULT 0,
    winner_id TEXT REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS battle_rounds (
    id TEXT PRIMARY KEY,
    battle_id TEXT NOT NULL REFERENCES battles(id),
    round INTEGER NOT NULL,
    p1_team JSONB NOT NULL DEFAULT '[]',
    p2_team JSONB NOT NULL DEFAULT '[]',
    p1_score DOUBLE PRECISION,
    p2_score DOUBLE PRECISION,
    winner_id TEXT REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_battles_player1 ON battles(player1_id);
CREATE INDEX IF NOT EXISTS idx_battles_player2 ON battles(player2_id);
CREATE INDEX IF NOT EXISTS idx_battles_status ON battles(status);
CREATE INDEX IF NOT EXISTS idx_battle_rounds_battle ON battle_rounds(battle_id);
CREATE INDEX IF NOT EXISTS idx_users_rating ON users(rating DESC);
