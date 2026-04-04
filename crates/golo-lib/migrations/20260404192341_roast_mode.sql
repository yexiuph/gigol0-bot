-- Add roast_mode to tracked_users table

ALTER TABLE tracked_users ADD COLUMN roast_mode BOOLEAN NOT NULL DEFAULT 0;
