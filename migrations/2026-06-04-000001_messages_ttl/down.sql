DROP INDEX IF EXISTS idx_messages_expires;
ALTER TABLE messages DROP COLUMN IF EXISTS expires_at;
