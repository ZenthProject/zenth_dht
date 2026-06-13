CREATE TABLE relay_messages (
    id                         BIGSERIAL PRIMARY KEY,
    for_device_dilithium_pubkey BYTEA NOT NULL,
    ciphertext                 BYTEA NOT NULL,
    nonce                      BYTEA NOT NULL,
    expires_at                 TIMESTAMP NOT NULL
);

CREATE INDEX idx_relay_device ON relay_messages (for_device_dilithium_pubkey, id);
