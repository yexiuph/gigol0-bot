-- Migration for tracking users and logging chats

CREATE TABLE IF NOT EXISTS tracked_users (
    user_id INTEGER PRIMARY KEY,
    reply_text TEXT,
    repeat_message BOOLEAN NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS chat_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);
