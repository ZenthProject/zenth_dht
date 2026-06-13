-- =============================================================================
-- Zenth DHT — Initial Schema
-- =============================================================================
-- Privacy-first, zero-knowledge design:
--   - No usernames, no emails, no IP addresses stored
--   - Users identified only by a hash (ZK proof commitment)
--   - All sensitive fields are opaque BYTEA blobs
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Users
-- ---------------------------------------------------------------------------
-- A user is identified solely by a hash of their identity (ZK commitment).
-- No personally identifiable information is stored server-side.
CREATE TABLE users (
    user_hash_id            BYTEA       PRIMARY KEY,
    password_commitment     BYTEA       NOT NULL,
    identity_key_dilithium  BYTEA       NOT NULL,
    identity_signature      BYTEA       NOT NULL,
    pre_key_bundle          BYTEA       NOT NULL,
    proof_type              INTEGER     NOT NULL,
    created_at              TIMESTAMP   NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Authentication
-- ---------------------------------------------------------------------------
-- Two-phase challenge/response login:
--   1. Client requests a challenge (nonce + proof parameters)
--   2. Client proves knowledge of secret without revealing it
CREATE TABLE auth_challenges (
    challenge_id        BYTEA       PRIMARY KEY,
    user_hash_id        BYTEA       NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    nonce               BYTEA       NOT NULL,
    required_proof_type INTEGER     NOT NULL,
    public_parameters   BYTEA       NOT NULL DEFAULT '',
    difficulty          INTEGER     NOT NULL DEFAULT 1,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),
    expires_at          TIMESTAMP   NOT NULL
);

CREATE INDEX idx_auth_challenges_user    ON auth_challenges(user_hash_id);
CREATE INDEX idx_auth_challenges_expires ON auth_challenges(expires_at);

-- Active sessions created after successful authentication
CREATE TABLE sessions (
    session_token   BYTEA       PRIMARY KEY,
    user_hash_id    BYTEA       NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    session_expiry  TIMESTAMP   NOT NULL,
    created_at      TIMESTAMP   NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sessions_user   ON sessions(user_hash_id);
CREATE INDEX idx_sessions_expiry ON sessions(session_expiry);

-- ---------------------------------------------------------------------------
-- Social graph
-- ---------------------------------------------------------------------------
-- Friend requests carry a pre-key bundle for X3DH key exchange and a
-- Dilithium signature so the recipient can verify authenticity.
CREATE TABLE friend_requests (
    id                  SERIAL      PRIMARY KEY,
    requester_hash_id   BYTEA       NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    target_hash_id      BYTEA       NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    pre_key_bundle      BYTEA       NOT NULL,
    dilithium_signature BYTEA       NOT NULL,
    encrypted_message   BYTEA,
    timestamp           BIGINT      NOT NULL,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),

    CONSTRAINT unique_friend_request UNIQUE (requester_hash_id, target_hash_id)
);

CREATE INDEX idx_friend_requests_target ON friend_requests(target_hash_id);

-- Responses include a pre-key bundle (accept path) for the initiating side
-- to complete X3DH, plus a Dilithium signature over the friendship.
CREATE TABLE friend_responses (
    id                  SERIAL      PRIMARY KEY,
    request_id          INTEGER     NOT NULL REFERENCES friend_requests(id) ON DELETE CASCADE,
    responder_hash_id   BYTEA       NOT NULL,
    requester_hash_id   BYTEA       NOT NULL,
    accepted            BOOLEAN     NOT NULL,
    pre_key_bundle      BYTEA,
    dilithium_signature BYTEA       NOT NULL,
    delivered           BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_friend_responses_requester    ON friend_responses(requester_hash_id);
CREATE INDEX idx_friend_responses_request_id   ON friend_responses(request_id);
CREATE INDEX idx_friend_responses_undelivered  ON friend_responses(requester_hash_id, delivered)
    WHERE delivered = FALSE;

-- ---------------------------------------------------------------------------
-- Messages
-- ---------------------------------------------------------------------------
-- End-to-end encrypted. The server stores opaque ciphertext and routes by
-- recipient hash. Messages are deleted once delivered.
CREATE TABLE messages (
    id                  SERIAL      PRIMARY KEY,
    message_id          BYTEA       NOT NULL UNIQUE,
    sender_hash_id      BYTEA       NOT NULL,
    recipient_hash_id   BYTEA       NOT NULL,
    content             BYTEA       NOT NULL,
    dilithium_signature BYTEA       NOT NULL,
    timestamp           BIGINT      NOT NULL,
    server_timestamp    BIGINT      NOT NULL,
    delivered           BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_messages_recipient ON messages(recipient_hash_id);
CREATE INDEX idx_messages_sender    ON messages(sender_hash_id);
