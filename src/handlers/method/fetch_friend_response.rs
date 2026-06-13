use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::{FriendResponseModel, User};
use crate::schema::{friend_responses, users};
use diesel::prelude::*;
use crate::timestamp::current_timestamp;
use zenth_dto::{
    FetchFriendResponsesRequest, FetchFriendResponsesResponse, FriendResponse as DtoFriendResponse,
};

pub async fn fetch_friend_responses(
    req: FetchFriendResponsesRequest,
) -> Result<FetchFriendResponsesResponse, String> {

    if req.user_hash.is_empty() || req.user_hash.len() != 32 {
        return Err("Invalid user hash".to_string());
    }

    if req.dilithium_signature.is_empty() {
        return Err("Dilithium signature is required".to_string());
    }

    let mut conn = establish_connection();

    let user = match users::table
        .filter(users::user_hash_id.eq(&req.user_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => return Err("User not found".to_string()),
        Err(e) => return Err(format!("Database error: {}", e)),
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

    let responses: Vec<FriendResponseModel> = if req.since_timestamp > 0 {
        friend_responses::table
            .filter(friend_responses::requester_hash_id.eq(&req.user_hash))
            .filter(
                friend_responses::created_at.gt(
                    chrono::DateTime::from_timestamp(req.since_timestamp as i64, 0)
                        .map(|dt| dt.naive_utc())
                        .unwrap_or_default()
                ),
            )
            .select(FriendResponseModel::as_select())
            .load(&mut conn)
            .map_err(|e| format!("Failed to fetch responses: {}", e))?
    } else {
        friend_responses::table
            .filter(friend_responses::requester_hash_id.eq(&req.user_hash))
            .select(FriendResponseModel::as_select())
            .load(&mut conn)
            .map_err(|e| format!("Failed to fetch responses: {}", e))?
    };


    let dto_responses: Vec<DtoFriendResponse> = responses
        .into_iter()
        .map(|r| DtoFriendResponse {
            responder_hash_id: r.responder_hash_id,
            requester_hash_id: r.requester_hash_id,
            accepted: r.accepted,
            pre_key_bundle: r.pre_key_bundle.unwrap_or_default(),
            dilithium_signature: r.dilithium_signature,
            timestamp: r.created_at.and_utc().timestamp() as u64,
        })
        .collect();

    Ok(FetchFriendResponsesResponse {
        responses: dto_responses,
        timestamp: current_timestamp(),
    })
}