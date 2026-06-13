ALTER TABLE messages
    ADD COLUMN expires_at TIMESTAMP NOT NULL DEFAULT (NOW() + INTERVAL '24 hours');

UPDATE messages SET expires_at = created_at + INTERVAL '24 hours';

CREATE INDEX idx_messages_expires ON messages (expires_at);
