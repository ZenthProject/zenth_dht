use diesel::prelude::*;
use zenth_dto::{
    RelayPushRequest, RelayPushResponse,
    RelayFetchRequest, RelayFetchResponse, RelayEntry,
    RelayAckRequest, RelayAckResponse,
};
use crate::db::establish_connection;
use crate::models::NewRelayMessage;
use crate::schema::relay_messages;
use crate::crypto::verify_dilithium_signature;
use crate::timestamp::is_timestamp_fresh;
use crate::rate_limit::relay_push_allowed;

const MAX_TTL: u64 = 86_400; // 24h
const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

pub async fn push_relay(req: RelayPushRequest) -> Result<RelayPushResponse, String> {
    if req.for_device_dilithium_pubkey.is_empty() {
        return Ok(RelayPushResponse { success: false, relay_id: 0, error_message: "destinataire vide".into() });
    }
    if req.ciphertext.is_empty() || req.nonce.len() != 12 {
        return Ok(RelayPushResponse { success: false, relay_id: 0, error_message: "ciphertext ou nonce invalide".into() });
    }
    if req.sender_dilithium_pubkey.is_empty() || req.sender_signature.is_empty() {
        return Ok(RelayPushResponse { success: false, relay_id: 0, error_message: "sender_dilithium_pubkey et sender_signature requis".into() });
    }
    if !is_timestamp_fresh(req.timestamp) {
        return Ok(RelayPushResponse { success: false, relay_id: 0, error_message: "Request timestamp expired".into() });
    }
    if !relay_push_allowed(&req.sender_dilithium_pubkey) {
        return Ok(RelayPushResponse { success: false, relay_id: 0, error_message: "Rate limit dépassé - réessayez dans 1 minute".into() });
    }

    // L'expéditeur prouve qu'il possède une clé Dilithium valide (anonyme, pas liée à un compte)
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&req.for_device_dilithium_pubkey);
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&req.sender_dilithium_pubkey, &signed_data, &req.sender_signature) {
        return Ok(RelayPushResponse { success: false, relay_id: 0, error_message: "Signature invalide".into() });
    }

    let ttl = req.ttl_secs.min(MAX_TTL).max(60);
    let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::seconds(ttl as i64);

    let new_msg = NewRelayMessage {
        for_device_dilithium_pubkey: &req.for_device_dilithium_pubkey,
        ciphertext: &req.ciphertext,
        nonce: &req.nonce,
        expires_at,
    };

    let mut conn = establish_connection();

    match diesel::insert_into(relay_messages::table)
        .values(&new_msg)
        .returning(relay_messages::id)
        .get_result::<i64>(&mut conn)
    {
        Ok(id) => Ok(RelayPushResponse { success: true, relay_id: id, error_message: String::new() }),
        Err(e) => Ok(RelayPushResponse { success: false, relay_id: 0, error_message: format!("DB error: {}", e) }),
    }
}

pub async fn fetch_relay(req: RelayFetchRequest) -> Result<RelayFetchResponse, String> {
    if req.for_device_dilithium_pubkey.is_empty() {
        return Ok(RelayFetchResponse { success: false, entries: vec![], error_message: "destinataire vide".into() });
    }
    if req.signature.is_empty() {
        return Ok(RelayFetchResponse { success: false, entries: vec![], error_message: "signature requise".into() });
    }
    if !is_timestamp_fresh(req.timestamp) {
        return Ok(RelayFetchResponse { success: false, entries: vec![], error_message: "Request timestamp expired".into() });
    }

    // Le device prouve qu'il est propriétaire de cette mailbox
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&req.for_device_dilithium_pubkey);
    signed_data.extend_from_slice(&req.since_id.to_le_bytes());
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&req.for_device_dilithium_pubkey, &signed_data, &req.signature) {
        return Ok(RelayFetchResponse { success: false, entries: vec![], error_message: "Signature invalide".into() });
    }

    let limit = match req.limit {
        0 => DEFAULT_LIMIT,
        n => n.min(MAX_LIMIT),
    };
    let now = chrono::Utc::now().naive_utc();
    let mut conn = establish_connection();

    let rows = relay_messages::table
        .filter(relay_messages::for_device_dilithium_pubkey.eq(&req.for_device_dilithium_pubkey))
        .filter(relay_messages::id.gt(req.since_id))
        .filter(relay_messages::expires_at.gt(now))
        .order(relay_messages::id.asc())
        .limit(limit as i64)
        .select((relay_messages::id, relay_messages::ciphertext, relay_messages::nonce))
        .load::<(i64, Vec<u8>, Vec<u8>)>(&mut conn)
        .map_err(|e| format!("DB error: {}", e))?;

    let entries = rows.into_iter().map(|(id, ciphertext, nonce)| RelayEntry { id, ciphertext, nonce }).collect();
    Ok(RelayFetchResponse { success: true, entries, error_message: String::new() })
}

pub async fn ack_relay(req: RelayAckRequest) -> Result<RelayAckResponse, String> {
    if req.for_device_dilithium_pubkey.is_empty() {
        return Ok(RelayAckResponse { success: false, error_message: "destinataire vide".into() });
    }
    if req.signature.is_empty() {
        return Ok(RelayAckResponse { success: false, error_message: "signature requise".into() });
    }
    if !is_timestamp_fresh(req.timestamp) {
        return Ok(RelayAckResponse { success: false, error_message: "Request timestamp expired".into() });
    }

    // Le device prouve qu'il est propriétaire de cette mailbox
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&req.for_device_dilithium_pubkey);
    signed_data.extend_from_slice(&req.up_to_id.to_le_bytes());
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&req.for_device_dilithium_pubkey, &signed_data, &req.signature) {
        return Ok(RelayAckResponse { success: false, error_message: "Signature invalide".into() });
    }

    let mut conn = establish_connection();

    match diesel::delete(
        relay_messages::table
            .filter(relay_messages::for_device_dilithium_pubkey.eq(&req.for_device_dilithium_pubkey))
            .filter(relay_messages::id.le(req.up_to_id))
    )
    .execute(&mut conn)
    {
        Ok(_) => Ok(RelayAckResponse { success: true, error_message: String::new() }),
        Err(e) => Ok(RelayAckResponse { success: false, error_message: format!("DB error: {}", e) }),
    }
}
