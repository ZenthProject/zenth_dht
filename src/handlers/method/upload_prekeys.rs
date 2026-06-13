use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::{NewOneTimePrekey, User};
use crate::schema::{one_time_prekeys, users};
use diesel::prelude::*;
use prost::Message;
use zenth_dto::{UploadPreKeysRequest, UploadPreKeysResponse};

pub async fn upload_prekeys(req: UploadPreKeysRequest) -> Result<UploadPreKeysResponse, String> {
    if req.username_hash.is_empty() || req.username_hash.len() != 32 {
        return Ok(UploadPreKeysResponse {
            success: false,
            prekeys_stored: 0,
            error_message: "Invalid username hash".to_string(),
        });
    }

    if req.auth_signature.is_empty() {
        return Ok(UploadPreKeysResponse {
            success: false,
            prekeys_stored: 0,
            error_message: "Auth signature required".to_string(),
        });
    }

    if !crate::timestamp::is_timestamp_fresh(req.timestamp) {
        return Ok(UploadPreKeysResponse {
            success: false,
            prekeys_stored: 0,
            error_message: "Request timestamp expired".to_string(),
        });
    }

    if req.one_time_prekeys.is_empty() {
        return Ok(UploadPreKeysResponse {
            success: false,
            prekeys_stored: 0,
            error_message: "No prekeys provided".to_string(),
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
            return Ok(UploadPreKeysResponse {
                success: false,
                prekeys_stored: 0,
                error_message: "User not found".to_string(),
            });
        }
        Err(e) => return Err(format!("Database error: {}", e)),
    };

    // Vérifie la signature : hash || timestamp
    let mut msg = Vec::new();
    msg.extend_from_slice(&req.username_hash);
    msg.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&user.identity_key_dilithium, &msg, &req.auth_signature) {
        return Ok(UploadPreKeysResponse {
            success: false,
            prekeys_stored: 0,
            error_message: "Invalid signature".to_string(),
        });
    }

    let new_prekeys: Vec<NewOneTimePrekey> = req
        .one_time_prekeys
        .iter()
        .map(|pk| NewOneTimePrekey {
            user_hash_id: &req.username_hash,
            prekey_id: pk.pre_key_id as i32,
            public_key: &pk.public_key,
        })
        .collect();

    let inserted = diesel::insert_into(one_time_prekeys::table)
        .values(&new_prekeys)
        .on_conflict((one_time_prekeys::user_hash_id, one_time_prekeys::prekey_id))
        .do_nothing()
        .execute(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(UploadPreKeysResponse {
        success: true,
        prekeys_stored: inserted as u32,
        error_message: String::new(),
    })
}
