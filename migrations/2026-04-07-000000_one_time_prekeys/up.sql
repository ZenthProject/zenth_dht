CREATE TABLE one_time_prekeys (
    id          SERIAL PRIMARY KEY,
    user_hash_id BYTEA   NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    prekey_id   INTEGER NOT NULL,
    public_key  BYTEA   NOT NULL,
    used        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (user_hash_id, prekey_id)
);

CREATE INDEX idx_otpk_user_unused
    ON one_time_prekeys (user_hash_id)
    WHERE NOT used;
