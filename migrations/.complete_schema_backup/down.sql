-- Rollback Zenth DHT Complete Database Schema
-- This file undoes the changes in up.sql

-- Drop views
DROP VIEW IF EXISTS user_statistics;
DROP VIEW IF EXISTS group_summary;
DROP VIEW IF EXISTS active_users;

-- Drop triggers
DROP TRIGGER IF EXISTS trigger_update_session_activity ON sessions;
DROP TRIGGER IF EXISTS trigger_update_group_member_count ON group_members;

-- Drop functions
DROP FUNCTION IF EXISTS update_session_activity();
DROP FUNCTION IF EXISTS update_group_member_count();
DROP FUNCTION IF EXISTS cleanup_expired_messages();
DROP FUNCTION IF EXISTS cleanup_delivered_messages();
DROP FUNCTION IF EXISTS cleanup_expired_group_invitations();
DROP FUNCTION IF EXISTS cleanup_expired_friend_requests();
DROP FUNCTION IF EXISTS cleanup_expired_challenges();

-- Drop tables in reverse order (respecting foreign key constraints)
DROP TABLE IF EXISTS audit_log CASCADE;
DROP TABLE IF EXISTS system_config CASCADE;
DROP TABLE IF EXISTS file_transfers CASCADE;
DROP TABLE IF EXISTS message_acks CASCADE;
DROP TABLE IF EXISTS message_queue CASCADE;
DROP TABLE IF EXISTS presence_status CASCADE;
DROP TABLE IF EXISTS peer_registry CASCADE;
DROP TABLE IF EXISTS sender_key_states CASCADE;
DROP TABLE IF EXISTS group_changes CASCADE;
DROP TABLE IF EXISTS group_invitations CASCADE;
DROP TABLE IF EXISTS group_members CASCADE;
DROP TABLE IF EXISTS groups CASCADE;
DROP TABLE IF EXISTS blocked_users CASCADE;
DROP TABLE IF EXISTS friend_responses CASCADE;
DROP TABLE IF EXISTS friend_requests CASCADE;
DROP TABLE IF EXISTS contacts CASCADE;
DROP TABLE IF EXISTS pq_pre_keys CASCADE;
DROP TABLE IF EXISTS signed_pre_keys CASCADE;
DROP TABLE IF EXISTS pre_keys CASCADE;
DROP TABLE IF EXISTS pre_key_bundles CASCADE;
DROP TABLE IF EXISTS auth_proofs_log CASCADE;
DROP TABLE IF EXISTS auth_challenges CASCADE;
DROP TABLE IF EXISTS sessions CASCADE;
DROP TABLE IF EXISTS users CASCADE;

-- Recreate old account table if needed (for backwards compatibility)
CREATE TABLE IF NOT EXISTS account (
  hash_id VARCHAR(128) NOT NULL,
  hash_mac VARCHAR(128) NOT NULL,
  code_otp VARCHAR(6)
);
