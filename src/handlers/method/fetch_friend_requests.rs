use diesel::prelude::*;
use zenth_dto::{FetchFriendRequestsRequest, FetchFriendRequestsResponse, FriendRequest as DtoFriendRequest};
use crate::db::establish_connection;
use crate::models::{User, FriendRequestModel};
use crate::schema::{users, friend_requests};
use crate::crypto::verify_dilithium_signature;
use crate::timestamp::current_timestamp;

pub async fn fetch_friend_requests(req: FetchFriendRequestsRequest) -> Result<FetchFriendRequestsResponse, String> {

    if req.user_hash.is_empty() {
        return Err("User hash is required".to_string());
    }

    if req.user_hash.len() != 32 {
        return Err(format!(
            "Invalid user hash length: expected 32, got {}",
            req.user_hash.len()
        ));
    }

    if req.dilithium_signature.is_empty() {
        return Err("Dilithium signature is required".to_string());
    }

    let mut conn = establish_connection();

    // Récupérer l'utilisateur pour vérifier la signature
    let user = match users::table
        .filter(users::user_hash_id.eq(&req.user_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Err("User not found".to_string());
        }
        Err(_) => {
            return Err("Database error".to_string());
        }
    };

    let mut message_to_verify = Vec::new();
    message_to_verify.extend_from_slice(&req.user_hash);
    message_to_verify.extend_from_slice(&req.since_timestamp.to_le_bytes());
    message_to_verify.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(
        &user.identity_key_dilithium,
        &message_to_verify,
        &req.dilithium_signature,
    ) {
        return Err("Invalid signature".to_string());
    }

    let requests_query = friend_requests::table
        .filter(friend_requests::target_hash_id.eq(&req.user_hash));

    let friend_request_models: Vec<FriendRequestModel> = if req.since_timestamp > 0 {
        requests_query
            .filter(friend_requests::timestamp.gt(req.since_timestamp as i64))
            .order(friend_requests::timestamp.desc())
            .select(FriendRequestModel::as_select())
            .load(&mut conn)
            .map_err(|e| format!("Failed to fetch friend requests: {}", e))?
    } else {
        requests_query
            .order(friend_requests::timestamp.desc())
            .select(FriendRequestModel::as_select())
            .load(&mut conn)
            .map_err(|e| format!("Failed to fetch friend requests: {}", e))?
    };

    let requests: Vec<DtoFriendRequest> = friend_request_models
        .into_iter()
        .map(|fr| DtoFriendRequest {
            requester_hash_id: fr.requester_hash_id,
            target_hash_id: fr.target_hash_id,
            pre_key_bundle: fr.pre_key_bundle,
            dilithium_signature: fr.dilithium_signature,
            encrypted_message: fr.encrypted_message.unwrap_or_default(),
            timestamp: fr.timestamp as u64,
        })
        .collect();

    let response = FetchFriendRequestsResponse {
        requests,
        timestamp: current_timestamp(),
    };

    Ok(response)
}
