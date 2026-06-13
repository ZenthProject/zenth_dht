use diesel::prelude::*;
use zenth_dto::{
    PublishRecoveryKeyRequest, PublishRecoveryKeyResponse,
    RecoveryClaimRequest, RecoveryClaimResponse,
};
use crate::db::establish_connection;
use crate::models::NewSession;
use crate::schema::{users, sessions};
use crate::crypto::verify_dilithium_signature;

// ── Method 29 : PUBLISH_RECOVERY_KEY ─────────────────────────────────────────

pub async fn publish_recovery_key(
    req: PublishRecoveryKeyRequest,
) -> Result<PublishRecoveryKeyResponse, String> {
    if req.username_hash.len() != 32 {
        return Err("username_hash invalide".to_string());
    }
    if req.recovery_dilithium_pubkey.is_empty() {
        return Err("recovery_dilithium_pubkey vide".to_string());
    }
    if req.auth_signature.is_empty() {
        return Err("auth_signature vide".to_string());
    }

    if !crate::timestamp::is_timestamp_fresh(req.timestamp) {
        return Err("Request timestamp expired".to_string());
    }

    let mut conn = establish_connection();

    // Load user and verify auth signature with main identity key
    let user = users::table
        .filter(users::user_hash_id.eq(&req.username_hash))
        .select(crate::models::User::as_select())
        .first::<crate::models::User>(&mut conn)
        .map_err(|_| "Utilisateur introuvable".to_string())?;

    // Signed message: username_hash || recovery_pubkey || timestamp (8 bytes LE)
    let mut signed_data = Vec::with_capacity(32 + req.recovery_dilithium_pubkey.len() + 8);
    signed_data.extend_from_slice(&req.username_hash);
    signed_data.extend_from_slice(&req.recovery_dilithium_pubkey);
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&user.identity_key_dilithium, &signed_data, &req.auth_signature) {
        return Ok(PublishRecoveryKeyResponse {
            success: false,
            error_message: "Signature invalide".to_string(),
        });
    }

    diesel::update(users::table.filter(users::user_hash_id.eq(&req.username_hash)))
        .set(users::recovery_dilithium_pubkey.eq(Some(&req.recovery_dilithium_pubkey)))
        .execute(&mut conn)
        .map_err(|e| format!("DB update: {}", e))?;

    Ok(PublishRecoveryKeyResponse { success: true, error_message: String::new() })
}

// ── Method 30 : RECOVERY_CLAIM ────────────────────────────────────────────────

pub async fn recovery_claim(
    req: RecoveryClaimRequest,
) -> Result<RecoveryClaimResponse, String> {
    if req.username_hash.len() != 32 {
        return Err("username_hash invalide".to_string());
    }
    if req.new_identity_dilithium_pubkey.is_empty() {
        return Err("new_identity_dilithium_pubkey vide".to_string());
    }
    if req.recovery_signature.is_empty() {
        return Err("recovery_signature vide".to_string());
    }

    if !crate::timestamp::is_timestamp_fresh(req.timestamp) {
        return Err("Request timestamp expired".to_string());
    }

    let mut conn = establish_connection();

    let user = users::table
        .filter(users::user_hash_id.eq(&req.username_hash))
        .select(crate::models::User::as_select())
        .first::<crate::models::User>(&mut conn)
        .map_err(|_| "Utilisateur introuvable".to_string())?;

    let recovery_pubkey = user.recovery_dilithium_pubkey
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| "Aucune clé de récupération enregistrée pour ce compte".to_string())?;

    // Signed message: username_hash || new_identity_pubkey || timestamp (8 bytes LE)
    let mut signed_data = Vec::with_capacity(32 + req.new_identity_dilithium_pubkey.len() + 8);
    signed_data.extend_from_slice(&req.username_hash);
    signed_data.extend_from_slice(&req.new_identity_dilithium_pubkey);
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(recovery_pubkey, &signed_data, &req.recovery_signature) {
        return Ok(RecoveryClaimResponse {
            success: false,
            session_token: vec![],
            error_message: "Signature de récupération invalide".to_string(),
        });
    }

    // Replace identity key + pre-key bundle
    diesel::update(users::table.filter(users::user_hash_id.eq(&req.username_hash)))
        .set((
            users::identity_key_dilithium.eq(&req.new_identity_dilithium_pubkey),
            users::identity_signature.eq(&req.new_identity_signature),
            users::pre_key_bundle.eq(&req.new_pre_key_bundle),
        ))
        .execute(&mut conn)
        .map_err(|e| format!("DB update identity: {}", e))?;

    // Issue a new session token
    let mut session_token_bytes = [0u8; 32];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut session_token_bytes);

    let expiry = chrono::Utc::now().naive_utc() + chrono::Duration::days(30);
    let new_session = NewSession {
        session_token: &session_token_bytes,
        user_hash_id: &req.username_hash,
        session_expiry: expiry,
    };

    diesel::insert_into(sessions::table)
        .values(&new_session)
        .execute(&mut conn)
        .map_err(|e| format!("DB insert session: {}", e))?;

    Ok(RecoveryClaimResponse {
        success: true,
        session_token: session_token_bytes.to_vec(),
        error_message: String::new(),
    })
}
