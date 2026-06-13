use diesel::prelude::*;
use zenth_dto::{SyncPushBlobRequest, SyncPushBlobResponse, SyncFetchBlobRequest, SyncFetchBlobResponse, SyncDeleteBlobRequest, SyncDeleteBlobResponse};
use crate::db::establish_connection;
use crate::models::NewSyncBlob;
use crate::schema::sync_blobs;
use crate::crypto::verify_dilithium_signature;
use crate::timestamp::is_timestamp_fresh;
use crate::rate_limit::blob_push_allowed;

pub async fn push_blob(req: SyncPushBlobRequest) -> Result<SyncPushBlobResponse, String> {
    if req.for_device_dilithium_pubkey.is_empty() {
        return Ok(SyncPushBlobResponse { success: false, error_message: "for_device_dilithium_pubkey vide".to_string() });
    }
    if req.ciphertext.is_empty() {
        return Ok(SyncPushBlobResponse { success: false, error_message: "ciphertext vide".to_string() });
    }
    if req.sender_dilithium_pubkey.is_empty() || req.auth_signature.is_empty() {
        return Ok(SyncPushBlobResponse { success: false, error_message: "sender_dilithium_pubkey et auth_signature requis".to_string() });
    }
    if !is_timestamp_fresh(req.timestamp) {
        return Ok(SyncPushBlobResponse { success: false, error_message: "Request timestamp expired".to_string() });
    }
    if !blob_push_allowed(&req.sender_dilithium_pubkey) {
        return Ok(SyncPushBlobResponse { success: false, error_message: "Rate limit dépassé - réessayez dans 1 minute".to_string() });
    }

    // L'expéditeur prouve qu'il possède une clé Dilithium valide (auth serveur)
    // auth_signature = sign(for_device_dilithium_pubkey || ciphertext || timestamp) par sender_dilithium_pubkey
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&req.for_device_dilithium_pubkey);
    signed_data.extend_from_slice(&req.ciphertext);
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&req.sender_dilithium_pubkey, &signed_data, &req.auth_signature) {
        return Ok(SyncPushBlobResponse { success: false, error_message: "Signature invalide".to_string() });
    }

    let ttl = req.ttl_secs.min(3600).max(60);
    let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::seconds(ttl as i64);

    let new_blob = NewSyncBlob {
        for_device_dilithium_pubkey: &req.for_device_dilithium_pubkey,
        ciphertext: &req.ciphertext,
        signature: &req.signature,
        expires_at,
    };

    let mut conn = establish_connection();

    match diesel::insert_into(sync_blobs::table)
        .values(&new_blob)
        .on_conflict(sync_blobs::for_device_dilithium_pubkey)
        .do_update()
        .set((
            sync_blobs::ciphertext.eq(&req.ciphertext),
            sync_blobs::signature.eq(&req.signature),
            sync_blobs::expires_at.eq(expires_at),
        ))
        .execute(&mut conn)
    {
        Ok(_) => Ok(SyncPushBlobResponse { success: true, error_message: String::new() }),
        Err(e) => Ok(SyncPushBlobResponse { success: false, error_message: format!("DB error: {}", e) }),
    }
}

pub async fn fetch_blob(req: SyncFetchBlobRequest) -> Result<SyncFetchBlobResponse, String> {
    if req.for_device_dilithium_pubkey.is_empty() {
        return Ok(SyncFetchBlobResponse { success: false, ciphertext: vec![], signature: vec![], error_message: "for_device_dilithium_pubkey vide".to_string() });
    }
    if req.signature.is_empty() {
        return Ok(SyncFetchBlobResponse { success: false, ciphertext: vec![], signature: vec![], error_message: "signature requise".to_string() });
    }
    if !is_timestamp_fresh(req.timestamp) {
        return Ok(SyncFetchBlobResponse { success: false, ciphertext: vec![], signature: vec![], error_message: "Request timestamp expired".to_string() });
    }

    // Le requester prouve qu'il possède une clé Dilithium valide
    // Si requester_dilithium_pubkey est fourni (cas pairing), on vérifie avec cette clé.
    // Sinon (cas normal), on vérifie avec for_device_dilithium_pubkey.
    let verify_key = if req.requester_dilithium_pubkey.is_empty() {
        &req.for_device_dilithium_pubkey
    } else {
        &req.requester_dilithium_pubkey
    };
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&req.for_device_dilithium_pubkey);
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(verify_key, &signed_data, &req.signature) {
        return Ok(SyncFetchBlobResponse { success: false, ciphertext: vec![], signature: vec![], error_message: "Signature invalide".to_string() });
    }

    let mut conn = establish_connection();
    let now = chrono::Utc::now().naive_utc();

    match sync_blobs::table
        .filter(sync_blobs::for_device_dilithium_pubkey.eq(&req.for_device_dilithium_pubkey))
        .filter(sync_blobs::expires_at.gt(now))
        .select((sync_blobs::ciphertext, sync_blobs::signature))
        .first::<(Vec<u8>, Vec<u8>)>(&mut conn)
    {
        Ok((ciphertext, signature)) => Ok(SyncFetchBlobResponse {
            success: true,
            ciphertext,
            signature,
            error_message: String::new(),
        }),
        Err(diesel::result::Error::NotFound) => Ok(SyncFetchBlobResponse {
            success: false,
            ciphertext: vec![],
            signature: vec![],
            error_message: "Blob introuvable ou expiré".to_string(),
        }),
        Err(e) => Ok(SyncFetchBlobResponse {
            success: false,
            ciphertext: vec![],
            signature: vec![],
            error_message: format!("DB error: {}", e),
        }),
    }
}

pub async fn delete_blob(req: SyncDeleteBlobRequest) -> Result<SyncDeleteBlobResponse, String> {
    if req.for_device_dilithium_pubkey.is_empty() {
        return Ok(SyncDeleteBlobResponse { success: false, error_message: "for_device_dilithium_pubkey vide".to_string() });
    }
    if req.signature.is_empty() {
        return Ok(SyncDeleteBlobResponse { success: false, error_message: "signature requise".to_string() });
    }
    if !is_timestamp_fresh(req.timestamp) {
        return Ok(SyncDeleteBlobResponse { success: false, error_message: "Request timestamp expired".to_string() });
    }

    let verify_key = if req.requester_dilithium_pubkey.is_empty() {
        &req.for_device_dilithium_pubkey
    } else {
        &req.requester_dilithium_pubkey
    };
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&req.for_device_dilithium_pubkey);
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(verify_key, &signed_data, &req.signature) {
        return Ok(SyncDeleteBlobResponse { success: false, error_message: "Signature invalide".to_string() });
    }

    let mut conn = establish_connection();

    match diesel::delete(
        sync_blobs::table.filter(sync_blobs::for_device_dilithium_pubkey.eq(&req.for_device_dilithium_pubkey))
    )
    .execute(&mut conn)
    {
        Ok(_) => Ok(SyncDeleteBlobResponse { success: true, error_message: String::new() }),
        Err(e) => Ok(SyncDeleteBlobResponse { success: false, error_message: format!("DB error: {}", e) }),
    }
}
