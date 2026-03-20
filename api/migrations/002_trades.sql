CREATE TABLE IF NOT EXISTS trades (
    id TEXT PRIMARY KEY,
    poster_id TEXT NOT NULL,
    offering_name TEXT NOT NULL,
    offering_types JSONB NOT NULL DEFAULT '[]',
    offering_power INTEGER NOT NULL DEFAULT 0,
    offering_shiny BOOLEAN NOT NULL DEFAULT false,
    looking_for TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() + INTERVAL '3 days'
);

CREATE TABLE IF NOT EXISTS trade_offers (
    id TEXT PRIMARY KEY,
    trade_id TEXT NOT NULL REFERENCES trades(id),
    offer_by_id TEXT NOT NULL,
    pokemon_name TEXT NOT NULL,
    pokemon_types JSONB NOT NULL DEFAULT '[]',
    pokemon_power INTEGER NOT NULL DEFAULT 0,
    pokemon_shiny BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);
CREATE INDEX IF NOT EXISTS idx_trades_poster ON trades(poster_id);
CREATE INDEX IF NOT EXISTS idx_trades_expires ON trades(expires_at);
CREATE INDEX IF NOT EXISTS idx_trade_offers_trade ON trade_offers(trade_id);
