use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::User;
use crate::schema::{one_time_prekeys, users};
use diesel::prelude::*;
use prost::Message;
use zenth_dto::{CheckPreKeyCountRequest, CheckPreKeyCountResponse, PreKeyBundle};

pub async fn check_prekey_count(req: CheckPreKeyCountRequest) -> Result<CheckPreKeyCountResponse, String> {
    if req.username_hash.is_empty() || req.username_hash.len() != 32 {
        return Ok(CheckPreKeyCountResponse {
            success: false,
            one_time_prekey_count: 0,
            has_signed_prekey: false,
            has_kyber_prekey: false,
            error_message: "Invalid username hash".to_string(),
        });
    }

    if req.auth_signature.is_empty() {
        return Ok(CheckPreKeyCountResponse {
            success: false,
            one_time_prekey_count: 0,
            has_signed_prekey: false,
            has_kyber_prekey: false,
            error_message: "Auth signature required".to_string(),
        });
    }

    let mut conn = establish_connection();

    let user = match users::table
        .filter(users::user_hash_id.eq(&req.username_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Ok(CheckPreKeyCountResponse {
                success: false,
                one_time_prekey_count: 0,
                has_signed_prekey: false,
                has_kyber_prekey: false,
                error_message: "User not found".to_string(),
            });
        }
        Err(e) => return Err(format!("Database error: {}", e)),
    };

    let mut msg = Vec::new();
    msg.extend_from_slice(&req.username_hash);
    msg.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&user.identity_key_dilithium, &msg, &req.auth_signature) {
        return Ok(CheckPreKeyCountResponse {
            success: false,
            one_time_prekey_count: 0,
            has_signed_prekey: false,
            has_kyber_prekey: false,
            error_message: "Invalid signature".to_string(),
        });
    }

    // Compte les one-time prekeys inutilisées
    let count: i64 = one_time_prekeys::table
        .filter(one_time_prekeys::user_hash_id.eq(&req.username_hash))
        .filter(one_time_prekeys::used.eq(false))
        .count()
        .get_result(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    // Vérifie la présence d'une signed prekey et d'une kyber prekey dans le bundle
    let bundle = PreKeyBundle::decode(user.pre_key_bundle.as_slice()).ok();
    let has_signed_prekey = bundle
        .as_ref()
        .map(|b| !b.signed_pre_key_public.is_empty())
        .unwrap_or(false);
    let has_kyber_prekey = bundle
        .as_ref()
        .map(|b| b.pq_pre_key_public.is_some())
        .unwrap_or(false);

    Ok(CheckPreKeyCountResponse {
        success: true,
        one_time_prekey_count: count as u32,
        has_signed_prekey,
        has_kyber_prekey,
        error_message: String::new(),
    })
}
