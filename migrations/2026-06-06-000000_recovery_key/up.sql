-- Add optional recovery Dilithium2 public key to users table.
-- NULL = user has not set up account recovery yet.
ALTER TABLE users
    ADD COLUMN recovery_dilithium_pubkey BYTEA DEFAULT NULL;
