CREATE TABLE sync_blobs (
    for_device_dilithium_pubkey BYTEA     PRIMARY KEY,
    ciphertext                  BYTEA     NOT NULL,
    signature                   BYTEA     NOT NULL,
    expires_at                  TIMESTAMP NOT NULL
);

CREATE INDEX idx_sync_blobs_expires ON sync_blobs (expires_at);
