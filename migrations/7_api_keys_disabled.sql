-- Add is_disabled column to api_keys table
ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS is_disabled BOOLEAN NOT NULL DEFAULT FALSE;
