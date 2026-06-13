use crate::schema::{auth_challenges, friend_requests, friend_responses, one_time_prekeys, relay_messages, sessions, sync_blobs, users};
use diesel::prelude::*;

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub user_hash_id: Vec<u8>,
    pub password_commitment: Vec<u8>,
    pub identity_key_dilithium: Vec<u8>,
    pub identity_signature: Vec<u8>,
    pub pre_key_bundle: Vec<u8>,
    pub proof_type: i32,
    pub created_at: chrono::NaiveDateTime,
    pub recovery_dilithium_pubkey: Option<Vec<u8>>,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub user_hash_id: &'a [u8],
    pub password_commitment: &'a [u8],
    pub identity_key_dilithium: &'a [u8],
    pub identity_signature: &'a [u8],
    pub pre_key_bundle: &'a [u8],
    pub proof_type: i32,
    pub recovery_dilithium_pubkey: Option<&'a [u8]>,
}

// Auth Challenge models
#[derive(Queryable, Selectable)]
#[diesel(table_name = auth_challenges)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AuthChallenge {
    pub challenge_id: Vec<u8>,
    pub user_hash_id: Vec<u8>,
    pub nonce: Vec<u8>,
    pub required_proof_type: i32,
    pub public_parameters: Vec<u8>,
    pub difficulty: i32,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = auth_challenges)]
pub struct NewAuthChallenge<'a> {
    pub challenge_id: &'a [u8],
    pub user_hash_id: &'a [u8],
    pub nonce: &'a [u8],
    pub required_proof_type: i32,
    pub public_parameters: &'a [u8],
    pub difficulty: i32,
    pub expires_at: chrono::NaiveDateTime,
}

// Session models
#[derive(Queryable, Selectable)]
#[diesel(table_name = sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Session {
    pub session_token: Vec<u8>,
    pub user_hash_id: Vec<u8>,
    pub session_expiry: chrono::NaiveDateTime,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = sessions)]
pub struct NewSession<'a> {
    pub session_token: &'a [u8],
    pub user_hash_id: &'a [u8],
    pub session_expiry: chrono::NaiveDateTime,
}

// Friend request models
#[derive(Queryable, Selectable)]
#[diesel(table_name = friend_requests)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FriendRequestModel {
    pub id: i32,
    pub requester_hash_id: Vec<u8>,
    pub target_hash_id: Vec<u8>,
    pub pre_key_bundle: Vec<u8>,
    pub dilithium_signature: Vec<u8>,
    pub encrypted_message: Option<Vec<u8>>,
    pub timestamp: i64,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = friend_requests)]
pub struct NewFriendRequest<'a> {
    pub requester_hash_id: &'a [u8],
    pub target_hash_id: &'a [u8],
    pub pre_key_bundle: &'a [u8],
    pub dilithium_signature: &'a [u8],
    pub encrypted_message: Option<&'a [u8]>,
    pub timestamp: i64,
}

// Friend response models
#[derive(Queryable, Selectable)]
#[diesel(table_name = friend_responses)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FriendResponseModel {
    pub id: i32,
    pub request_id: i32,
    pub responder_hash_id: Vec<u8>,
    pub requester_hash_id: Vec<u8>,
    pub accepted: bool,
    pub pre_key_bundle: Option<Vec<u8>>,
    pub dilithium_signature: Vec<u8>,
    pub delivered: bool,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = friend_responses)]
pub struct NewFriendResponse<'a> {
    pub request_id: i32,
    pub responder_hash_id: &'a [u8],
    pub requester_hash_id: &'a [u8],
    pub accepted: bool,
    pub pre_key_bundle: Option<&'a [u8]>,
    pub dilithium_signature: &'a [u8],
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = one_time_prekeys)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OneTimePrekey {
    pub id:           i32,
    pub user_hash_id: Vec<u8>,
    pub prekey_id:    i32,
    pub public_key:   Vec<u8>,
    pub used:         bool,
    pub created_at:   chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = one_time_prekeys)]
pub struct NewOneTimePrekey<'a> {
    pub user_hash_id: &'a [u8],
    pub prekey_id:    i32,
    pub public_key:   &'a [u8],
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::messages)]
pub struct NewMessage<'a> {
    pub message_id: &'a [u8],
    pub sender_hash_id: &'a [u8],
    pub recipient_hash_id: &'a [u8],
    pub content: &'a [u8],
    pub dilithium_signature: &'a [u8],
    pub timestamp: i64,
    pub server_timestamp: i64,
    pub expires_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MessageModel {
    pub id: i32,
    pub message_id: Vec<u8>,
    pub sender_hash_id: Vec<u8>,
    pub recipient_hash_id: Vec<u8>,
    pub content: Vec<u8>,
    pub dilithium_signature: Vec<u8>,
    pub timestamp: i64,
    pub server_timestamp: i64,
    pub delivered: bool,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = relay_messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RelayMessage {
    pub id:                          i64,
    pub for_device_dilithium_pubkey: Vec<u8>,
    pub ciphertext:                  Vec<u8>,
    pub nonce:                       Vec<u8>,
    pub expires_at:                  chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = relay_messages)]
pub struct NewRelayMessage<'a> {
    pub for_device_dilithium_pubkey: &'a [u8],
    pub ciphertext:                  &'a [u8],
    pub nonce:                       &'a [u8],
    pub expires_at:                  chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = sync_blobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SyncBlob {
    pub for_device_dilithium_pubkey: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub signature: Vec<u8>,
    pub expires_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = sync_blobs)]
pub struct NewSyncBlob<'a> {
    pub for_device_dilithium_pubkey: &'a [u8],
    pub ciphertext: &'a [u8],
    pub signature: &'a [u8],
    pub expires_at: chrono::NaiveDateTime,
}
