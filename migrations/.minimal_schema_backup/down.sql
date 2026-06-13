-- Rollback Zenth DHT Minimal Schema

-- Drop views
DROP VIEW IF EXISTS group_summary;
DROP VIEW IF EXISTS active_users;

-- Drop triggers
DROP TRIGGER IF EXISTS trigger_update_group_member_count ON group_members;

-- Drop functions
DROP FUNCTION IF EXISTS update_group_member_count();
DROP FUNCTION IF EXISTS cleanup_expired_messages();
DROP FUNCTION IF EXISTS cleanup_expired_group_invitations();
DROP FUNCTION IF EXISTS cleanup_expired_friend_requests();
DROP FUNCTION IF EXISTS cleanup_expired_challenges();

-- Drop tables in reverse order
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
DROP TABLE IF EXISTS auth_challenges CASCADE;
DROP TABLE IF EXISTS sessions CASCADE;
DROP TABLE IF EXISTS users CASCADE;
