-- Zenth DHT Minimal Database Schema
-- Version: 2.0 - STRICT MINIMUM
-- Date: 2025-12-10

-- Drop old tables
DROP TABLE IF EXISTS account CASCADE;
DROP TABLE IF EXISTS audit_log CASCADE;
DROP TABLE IF EXISTS auth_proofs_log CASCADE;

-- ============================================================================
-- SECTION 1: USERS AND AUTHENTICATION
-- ============================================================================

-- Main users table - MINIMAL
CREATE TABLE users (
    user_hash_id BYTEA PRIMARY KEY,
    password_commitment BYTEA NOT NULL,
    proof_type INTEGER NOT NULL,
    identity_key_dilithium BYTEA NOT NULL,
    identity_signature BYTEA NOT NULL,
    challenge_parameters BYTEA,
    registration_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX idx_users_created_at ON users(created_at);

-- Active authentication sessions - MINIMAL
CREATE TABLE sessions (
    session_token BYTEA PRIMARY KEY,
    user_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    session_expiry TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sessions_user_hash_id ON sessions(user_hash_id);
CREATE INDEX idx_sessions_expiry ON sessions(session_expiry);

-- Temporary authentication challenges - MINIMAL
CREATE TABLE auth_challenges (
    challenge_id BYTEA PRIMARY KEY,
    user_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    nonce BYTEA NOT NULL,
    required_proof_type INTEGER NOT NULL,
    public_parameters BYTEA NOT NULL,
    difficulty INTEGER NOT NULL DEFAULT 1,
    expires_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_challenges_user_hash_id ON auth_challenges(user_hash_id);
CREATE INDEX idx_challenges_expires_at ON auth_challenges(expires_at);

-- ============================================================================
-- SECTION 2: CRYPTOGRAPHIC KEYS
-- ============================================================================

-- Complete pre-key bundles for X3DH
CREATE TABLE pre_key_bundles (
    user_hash_id BYTEA PRIMARY KEY REFERENCES users(user_hash_id) ON DELETE CASCADE,
    registration_id INTEGER NOT NULL,
    identity_key_type INTEGER NOT NULL,
    identity_key_public BYTEA NOT NULL,
    bundle_version INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Ephemeral pre-keys (X25519)
CREATE TABLE pre_keys (
    id BIGSERIAL PRIMARY KEY,
    user_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    pre_key_id INTEGER NOT NULL,
    public_key BYTEA NOT NULL,
    key_type INTEGER NOT NULL DEFAULT 1,
    is_consumed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(user_hash_id, pre_key_id)
);

CREATE INDEX idx_pre_keys_user_hash_id ON pre_keys(user_hash_id);
CREATE INDEX idx_pre_keys_available ON pre_keys(user_hash_id, is_consumed) WHERE is_consumed = FALSE;

-- Signed pre-keys with Dilithium
CREATE TABLE signed_pre_keys (
    id BIGSERIAL PRIMARY KEY,
    user_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    signed_pre_key_id INTEGER NOT NULL,
    public_key BYTEA NOT NULL,
    signature BYTEA NOT NULL,
    signature_type INTEGER NOT NULL DEFAULT 2,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(user_hash_id, signed_pre_key_id)
);

CREATE INDEX idx_signed_pre_keys_user_hash_id ON signed_pre_keys(user_hash_id);
CREATE INDEX idx_signed_pre_keys_active ON signed_pre_keys(user_hash_id, is_active) WHERE is_active = TRUE;

-- Post-quantum KEM pre-keys (Kyber/MLKEM)
CREATE TABLE pq_pre_keys (
    id BIGSERIAL PRIMARY KEY,
    user_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    pq_pre_key_id INTEGER NOT NULL,
    key_type INTEGER NOT NULL,
    public_key BYTEA NOT NULL,
    is_consumed BOOLEAN NOT NULL DEFAULT FALSE,
    is_last_resort BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(user_hash_id, pq_pre_key_id)
);

CREATE INDEX idx_pq_pre_keys_user_hash_id ON pq_pre_keys(user_hash_id);
CREATE INDEX idx_pq_pre_keys_available ON pq_pre_keys(user_hash_id, is_consumed) WHERE is_consumed = FALSE;
CREATE INDEX idx_pq_pre_keys_last_resort ON pq_pre_keys(user_hash_id, is_last_resort) WHERE is_last_resort = TRUE;

-- ============================================================================
-- SECTION 3: CONTACTS AND RELATIONSHIPS
-- ============================================================================

-- User contacts - MINIMAL
CREATE TABLE contacts (
    id BIGSERIAL PRIMARY KEY,
    owner_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    contact_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    trust_level INTEGER NOT NULL DEFAULT 0,
    verification_signature BYTEA,
    added_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(owner_hash_id, contact_hash_id),
    CHECK (owner_hash_id != contact_hash_id)
);

CREATE INDEX idx_contacts_owner_hash_id ON contacts(owner_hash_id);
CREATE INDEX idx_contacts_contact_hash_id ON contacts(contact_hash_id);

-- Pending friend requests
CREATE TABLE friend_requests (
    id BIGSERIAL PRIMARY KEY,
    requester_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    target_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    pre_key_bundle BYTEA NOT NULL,
    dilithium_signature BYTEA NOT NULL,
    encrypted_message BYTEA,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    UNIQUE(requester_hash_id, target_hash_id),
    CHECK (requester_hash_id != target_hash_id),
    CHECK (status IN ('pending', 'accepted', 'rejected', 'expired'))
);

CREATE INDEX idx_friend_requests_target_hash_id ON friend_requests(target_hash_id, status);
CREATE INDEX idx_friend_requests_pending ON friend_requests(target_hash_id, status) WHERE status = 'pending';
CREATE INDEX idx_friend_requests_expires_at ON friend_requests(expires_at) WHERE status = 'pending';

-- Friend request responses
CREATE TABLE friend_responses (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT NOT NULL REFERENCES friend_requests(id) ON DELETE CASCADE,
    responder_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    requester_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    accepted BOOLEAN NOT NULL,
    pre_key_bundle BYTEA,
    dilithium_signature BYTEA NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    delivered BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_friend_responses_requester_hash_id ON friend_responses(requester_hash_id, delivered);

-- Blocked users
CREATE TABLE blocked_users (
    id BIGSERIAL PRIMARY KEY,
    blocker_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    blocked_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    blocked_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(blocker_hash_id, blocked_hash_id),
    CHECK (blocker_hash_id != blocked_hash_id)
);

CREATE INDEX idx_blocked_users_blocker_hash_id ON blocked_users(blocker_hash_id);

-- ============================================================================
-- SECTION 4: GROUPS
-- ============================================================================

-- Groups with LMS keys
CREATE TABLE groups (
    group_id BYTEA PRIMARY KEY,
    group_name VARCHAR(255) NOT NULL,
    group_type INTEGER NOT NULL,
    creator_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE RESTRICT,
    master_lms_key_type INTEGER NOT NULL,
    master_lms_key_public BYTEA NOT NULL,
    tree_identifier BYTEA NOT NULL,
    sender_key_chain_id INTEGER NOT NULL DEFAULT 0,
    member_count INTEGER NOT NULL DEFAULT 0,
    max_members INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CHECK (group_type IN (1, 2, 3)),
    CHECK (member_count >= 0)
);

CREATE INDEX idx_groups_creator_hash_id ON groups(creator_hash_id);
CREATE INDEX idx_groups_type ON groups(group_type, is_active);

-- Group members with roles
CREATE TABLE group_members (
    id BIGSERIAL PRIMARY KEY,
    group_id BYTEA NOT NULL REFERENCES groups(group_id) ON DELETE CASCADE,
    user_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    role INTEGER NOT NULL DEFAULT 1,
    member_lms_key_type INTEGER NOT NULL,
    member_lms_key_public BYTEA NOT NULL,
    pre_key_bundle BYTEA NOT NULL,
    joined_at TIMESTAMP NOT NULL DEFAULT NOW(),
    invited_by BYTEA REFERENCES users(user_hash_id) ON DELETE SET NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE(group_id, user_hash_id),
    CHECK (role IN (1, 2, 3))
);

CREATE INDEX idx_group_members_group_id ON group_members(group_id, is_active);
CREATE INDEX idx_group_members_user_hash_id ON group_members(user_hash_id);

-- Group invitations
CREATE TABLE group_invitations (
    id BIGSERIAL PRIMARY KEY,
    group_id BYTEA NOT NULL REFERENCES groups(group_id) ON DELETE CASCADE,
    inviter_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    invitee_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    encrypted_group_state BYTEA NOT NULL,
    invitee_lms_key_type INTEGER NOT NULL,
    invitee_lms_key_public BYTEA NOT NULL,
    lms_signature BYTEA NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    UNIQUE(group_id, invitee_hash_id, created_at),
    CHECK (status IN ('pending', 'accepted', 'rejected', 'expired'))
);

CREATE INDEX idx_group_invitations_invitee_hash_id ON group_invitations(invitee_hash_id, status);
CREATE INDEX idx_group_invitations_pending ON group_invitations(invitee_hash_id, status) WHERE status = 'pending';

-- Group state change log
CREATE TABLE group_changes (
    id BIGSERIAL PRIMARY KEY,
    group_id BYTEA NOT NULL REFERENCES groups(group_id) ON DELETE CASCADE,
    operation INTEGER NOT NULL,
    initiator_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    change_details JSONB NOT NULL,
    lms_signature BYTEA NOT NULL,
    previous_version INTEGER NOT NULL,
    new_version INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_group_changes_group_id ON group_changes(group_id, new_version DESC);

-- Sender key states for group messaging
CREATE TABLE sender_key_states (
    id BIGSERIAL PRIMARY KEY,
    group_id BYTEA NOT NULL REFERENCES groups(group_id) ON DELETE CASCADE,
    sender_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    sender_key_id INTEGER NOT NULL,
    chain_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    chain_key BYTEA NOT NULL,
    signing_key BYTEA NOT NULL,
    lms_signature BYTEA NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE(group_id, sender_hash_id, chain_id)
);

CREATE INDEX idx_sender_key_states_group_id ON sender_key_states(group_id, is_active);

-- ============================================================================
-- SECTION 5: P2P NETWORK AND ROUTING
-- ============================================================================

-- Peer registry for DHT routing - MINIMAL
CREATE TABLE peer_registry (
    user_hash_id BYTEA PRIMARY KEY REFERENCES users(user_hash_id) ON DELETE CASCADE,
    connection_public_key BYTEA NOT NULL,
    network_addresses JSONB NOT NULL,
    is_online BOOLEAN NOT NULL DEFAULT FALSE,
    last_seen_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_peer_registry_is_online ON peer_registry(is_online, last_seen_at);

-- Presence status - MINIMAL
CREATE TABLE presence_status (
    id BIGSERIAL PRIMARY KEY,
    user_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    status INTEGER NOT NULL,
    connected_peers BYTEA[],
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    dilithium_signature BYTEA NOT NULL,
    CHECK (status IN (1, 2, 3, 4))
);

CREATE INDEX idx_presence_status_user_hash_id ON presence_status(user_hash_id, updated_at DESC);

-- Message queue for offline delivery - MINIMAL
CREATE TABLE message_queue (
    id BIGSERIAL PRIMARY KEY,
    message_id BYTEA NOT NULL UNIQUE,
    sender_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    recipient_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    message_type INTEGER NOT NULL,
    encrypted_envelope BYTEA NOT NULL,
    priority INTEGER NOT NULL DEFAULT 5,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP NOT NULL,
    is_delivered BOOLEAN NOT NULL DEFAULT FALSE,
    CHECK (priority BETWEEN 1 AND 10)
);

CREATE INDEX idx_message_queue_recipient_hash_id ON message_queue(recipient_hash_id, is_delivered);
CREATE INDEX idx_message_queue_pending ON message_queue(recipient_hash_id, created_at) WHERE is_delivered = FALSE;
CREATE INDEX idx_message_queue_priority ON message_queue(priority DESC, created_at) WHERE is_delivered = FALSE;

-- Message acknowledgments
CREATE TABLE message_acks (
    id BIGSERIAL PRIMARY KEY,
    message_id BYTEA NOT NULL,
    recipient_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    delivered BOOLEAN NOT NULL DEFAULT FALSE,
    read BOOLEAN NOT NULL DEFAULT FALSE,
    delivered_at TIMESTAMP,
    read_at TIMESTAMP,
    dilithium_signature BYTEA NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_message_acks_message_id ON message_acks(message_id);

-- ============================================================================
-- SECTION 6: FILE TRANSFERS
-- ============================================================================

-- File transfer coordination - MINIMAL
CREATE TABLE file_transfers (
    transfer_id BYTEA PRIMARY KEY,
    sender_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    recipient_hash_id BYTEA NOT NULL REFERENCES users(user_hash_id) ON DELETE CASCADE,
    filename VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    file_hash BYTEA NOT NULL,
    total_chunks INTEGER NOT NULL,
    chunks_transferred INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'offer',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CHECK (file_size > 0),
    CHECK (total_chunks > 0),
    CHECK (chunks_transferred >= 0),
    CHECK (status IN ('offer', 'accept', 'reject', 'transferring', 'complete', 'error'))
);

CREATE INDEX idx_file_transfers_recipient_hash_id ON file_transfers(recipient_hash_id, status);

-- ============================================================================
-- SECTION 7: SYSTEM
-- ============================================================================

-- System configuration
CREATE TABLE system_config (
    key VARCHAR(100) PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Insert minimal configuration
INSERT INTO system_config (key, value, description) VALUES
    ('schema_version', '2.0', 'Database schema version (MINIMAL)'),
    ('max_challenge_ttl_seconds', '300', 'Maximum TTL for auth challenges (5 minutes)'),
    ('max_session_duration_hours', '168', 'Maximum session duration (7 days)'),
    ('max_message_queue_days', '30', 'Maximum days to keep undelivered messages'),
    ('max_prekeys_per_user', '100', 'Maximum number of pre-keys per user');

-- ============================================================================
-- CLEANUP FUNCTIONS
-- ============================================================================

-- Clean expired challenges
CREATE OR REPLACE FUNCTION cleanup_expired_challenges()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM auth_challenges WHERE expires_at < NOW();
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Clean expired friend requests
CREATE OR REPLACE FUNCTION cleanup_expired_friend_requests()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    UPDATE friend_requests
    SET status = 'expired'
    WHERE status = 'pending' AND expires_at < NOW();
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Clean expired group invitations
CREATE OR REPLACE FUNCTION cleanup_expired_group_invitations()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    UPDATE group_invitations
    SET status = 'expired'
    WHERE status = 'pending' AND expires_at < NOW();
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Clean expired undelivered messages
CREATE OR REPLACE FUNCTION cleanup_expired_messages()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM message_queue
    WHERE is_delivered = FALSE AND expires_at < NOW();
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TRIGGERS
-- ============================================================================

-- Update group member count trigger
CREATE OR REPLACE FUNCTION update_group_member_count()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE groups
        SET member_count = member_count + 1,
            updated_at = NOW()
        WHERE group_id = NEW.group_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE groups
        SET member_count = member_count - 1,
            updated_at = NOW()
        WHERE group_id = OLD.group_id;
    ELSIF TG_OP = 'UPDATE' AND OLD.is_active = TRUE AND NEW.is_active = FALSE THEN
        UPDATE groups
        SET member_count = member_count - 1,
            updated_at = NOW()
        WHERE group_id = NEW.group_id;
    ELSIF TG_OP = 'UPDATE' AND OLD.is_active = FALSE AND NEW.is_active = TRUE THEN
        UPDATE groups
        SET member_count = member_count + 1,
            updated_at = NOW()
        WHERE group_id = NEW.group_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_group_member_count
AFTER INSERT OR DELETE OR UPDATE OF is_active ON group_members
FOR EACH ROW
EXECUTE FUNCTION update_group_member_count();

-- ============================================================================
-- VIEWS
-- ============================================================================

-- Active users with online status
CREATE VIEW active_users AS
SELECT
    u.user_hash_id,
    u.registration_id,
    u.created_at,
    pr.is_online,
    pr.last_seen_at,
    ps.status as presence_status
FROM users u
LEFT JOIN peer_registry pr ON u.user_hash_id = pr.user_hash_id
LEFT JOIN LATERAL (
    SELECT status
    FROM presence_status
    WHERE user_hash_id = u.user_hash_id
    ORDER BY updated_at DESC
    LIMIT 1
) ps ON TRUE
WHERE u.is_active = TRUE;

-- Group summary
CREATE VIEW group_summary AS
SELECT
    g.group_id,
    g.group_name,
    g.group_type,
    g.creator_hash_id,
    g.member_count,
    g.version,
    g.is_active,
    g.created_at,
    g.updated_at,
    COUNT(DISTINCT gm.user_hash_id) FILTER (WHERE gm.is_active = TRUE) as active_members,
    COUNT(DISTINCT gi.id) FILTER (WHERE gi.status = 'pending') as pending_invitations
FROM groups g
LEFT JOIN group_members gm ON g.group_id = gm.group_id
LEFT JOIN group_invitations gi ON g.group_id = gi.group_id
GROUP BY g.group_id;

-- ============================================================================
-- END OF MINIMAL SCHEMA
-- ============================================================================

COMMENT ON DATABASE zenth_dht IS 'Zenth DHT - Minimal schema v2.0';
COMMENT ON TABLE users IS 'User accounts with ZKP authentication (NO username_hash, NO tracking)';
COMMENT ON TABLE sessions IS 'Active sessions (NO IP tracking, NO user_agent)';
COMMENT ON TABLE auth_challenges IS 'Temporary ZKP challenges (expires in 5 min)';
