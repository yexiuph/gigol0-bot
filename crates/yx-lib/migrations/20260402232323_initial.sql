CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    dicord_id INTEGER NOT NULL,
);

CREATE INDEX IF NOT EXISTS idx_user_id(id);

CREATE TABLE IF NOT EXISTS wallets (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    points INTEGER NOT NULL DEFAULT 0,
);

CREATE INDEX IF NOT EXISTS idx_wallet_user_id(user_id);

CREATE TABLE IF NOT EXISTS referrals (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    referred_id INTEGER REFERENCES users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
);

CREATE INDEX IF NOT EXISTS idx_referrals_user_id(user_id);
CREATE INDEX IF NOT EXISTS idx_referrals_referred_id(referred_id);

CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY,
    actor INTEGER REFERENCES users(id),
    action INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
);

CREATE INDEX IF NOT EXISTS idx_transactions_actor(actor);
CREATE INDEX IF NOT EXISTS idx_transactions_action(action);