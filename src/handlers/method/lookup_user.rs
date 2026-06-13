use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::User;
use crate::schema::users;
use diesel::prelude::*;
use prost::Message;
use zenth_dto::{LookupUserRequest, LookupUserResponse, PreKeyBundle};

pub async fn lookup_user(req: LookupUserRequest) -> Result<LookupUserResponse, String> {

    if req.requester_hash.is_empty() || req.requester_hash.len() != 32 {
        return Err("Invalid requester hash".to_string());
    }

    if req.target_hash.is_empty() || req.target_hash.len() != 32 {
        return Err("Invalid target hash".to_string());
    }

    if req.dilithium_signature.is_empty() {
        return Err("Dilithium signature is required".to_string());
    }

    let mut conn = establish_connection();

    let requester = match users::table
        .filter(users::user_hash_id.eq(&req.requester_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Err("Requester not found".to_string());
        }
        Err(e) => {
            return Err(format!("Database error: {}", e));
        }
    };

    let mut message_to_verify = Vec::new();
    message_to_verify.extend_from_slice(&req.requester_hash);
    message_to_verify.extend_from_slice(&req.target_hash);
    message_to_verify.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(
        &requester.identity_key_dilithium,
        &message_to_verify,
        &req.dilithium_signature,
    ) {
        return Err("Invalid signature".to_string());
    }

    let target = match users::table
        .filter(users::user_hash_id.eq(&req.target_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Ok(LookupUserResponse {
                found: false,
                user_hash: vec![],
                pre_key_bundle: None,
                error_message: String::new(),
            });
        }
        Err(e) => {
            return Err(format!("Database error: {}", e));
        }
    };

    let pre_key_bundle = match PreKeyBundle::decode(target.pre_key_bundle.as_slice()) {
        Ok(bundle) => bundle,
        Err(_) => {
            return Err("Failed to decode user's PreKeyBundle".to_string());
        }
    };

    Ok(LookupUserResponse {
        found: true,
        user_hash: target.user_hash_id,
        pre_key_bundle: Some(pre_key_bundle),
        error_message: String::new(),
    })
}
