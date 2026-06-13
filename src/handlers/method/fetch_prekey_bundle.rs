use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::User;
use crate::schema::{users, one_time_prekeys};
use diesel::prelude::*;
use prost::Message;
use zenth_dto::{FetchPreKeyBundleRequest, FetchPreKeyBundleResponse, PreKeyBundle};

pub async fn fetch_prekey_bundle(req: FetchPreKeyBundleRequest) -> Result<FetchPreKeyBundleResponse, String> {

    if req.requester_hash.is_empty() || req.requester_hash.len() != 32 {
        return Ok(FetchPreKeyBundleResponse {
            success: false,
            bundle: None,
            error_message: "Invalid requester hash".to_string(),
        });
    }

    if req.target_hash.is_empty() || req.target_hash.len() != 32 {
        return Ok(FetchPreKeyBundleResponse {
            success: false,
            bundle: None,
            error_message: "Invalid target hash".to_string(),
        });
    }

    if req.auth_signature.is_empty() {
        return Ok(FetchPreKeyBundleResponse {
            success: false,
            bundle: None,
            error_message: "Auth signature is required".to_string(),
        });
    }

    let mut conn = establish_connection();

    let requester = match users::table
        .filter(users::user_hash_id.eq(&req.requester_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Ok(FetchPreKeyBundleResponse {
                success: false,
                bundle: None,
                error_message: "Requester not found".to_string(),
            });
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
        &req.auth_signature,
    ) {
        return Ok(FetchPreKeyBundleResponse {
            success: false,
            bundle: None,
            error_message: "Invalid signature".to_string(),
        });
    }

    let target = match users::table
        .filter(users::user_hash_id.eq(&req.target_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Ok(FetchPreKeyBundleResponse {
                success: false,
                bundle: None,
                error_message: "Target user not found".to_string(),
            });
        }
        Err(e) => {
            return Err(format!("Database error: {}", e));
        }
    };

    // 4. Décoder le PreKeyBundle de base (IK + SPK)
    let mut bundle = match PreKeyBundle::decode(target.pre_key_bundle.as_slice()) {
        Ok(b) => b,
        Err(_) => {
            return Ok(FetchPreKeyBundleResponse {
                success: false,
                bundle: None,
                error_message: "Failed to decode user's PreKeyBundle".to_string(),
            });
        }
    };

    // 5. Piocher une OTPK non utilisée et la marquer comme consommée
    let otpk = one_time_prekeys::table
        .filter(one_time_prekeys::user_hash_id.eq(&req.target_hash))
        .filter(one_time_prekeys::used.eq(false))
        .order(one_time_prekeys::id.asc())
        .select((one_time_prekeys::id, one_time_prekeys::prekey_id, one_time_prekeys::public_key))
        .first::<(i32, i32, Vec<u8>)>(&mut conn)
        .optional()
        .map_err(|e| format!("Database error: {}", e))?;

    if let Some((row_id, prekey_id, public_key)) = otpk {
        diesel::update(one_time_prekeys::table.find(row_id))
            .set(one_time_prekeys::used.eq(true))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to mark OTPK as used: {}", e))?;

        bundle.pre_key_id = prekey_id as u32;
        bundle.pre_key_public = public_key;
    }
    // Si plus d'OTPK disponible : on retourne le bundle sans OTPK (pre_key_id = 0)
    // Le client devra établir la session sans forward secrecy maximale

    Ok(FetchPreKeyBundleResponse {
        success: true,
        bundle: Some(bundle),
        error_message: String::new(),
    })
}
