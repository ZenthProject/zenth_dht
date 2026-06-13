use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::User;
use crate::schema::{users, friend_requests, friend_responses};
use crate::timestamp::current_timestamp;
use diesel::prelude::*;
use zenth_dto::{FetchFriendResponsesRequest, FetchFriendResponsesResponse, FriendResponse as DtoFriendResponse};

/// Retourne toutes les demandes d'ami que l'utilisateur a acceptées (il est responder).
///
/// Cas non couvert par fetch_friend_responses (qui ne cherche que WHERE requester = user).
/// Ici on cherche WHERE responder = user AND accepted = true, et on retourne le pre_key_bundle
/// du REQUESTER (depuis friend_requests) pour que l'autre appareil puisse reconstruire le contact.
pub async fn fetch_my_accepted(
    req: FetchFriendResponsesRequest,
) -> Result<FetchFriendResponsesResponse, String> {
    if req.user_hash.is_empty() || req.user_hash.len() != 32 {
        return Err("Invalid user hash".to_string());
    }
    if req.dilithium_signature.is_empty() {
        return Err("Dilithium signature required".to_string());
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

    let mut msg = Vec::new();
    msg.extend_from_slice(&req.user_hash);
    msg.extend_from_slice(&req.since_timestamp.to_le_bytes());
    msg.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&user.identity_key_dilithium, &msg, &req.dilithium_signature) {
        return Err("Invalid signature".to_string());
    }

    // JOIN friend_requests pour récupérer le pre_key_bundle du requester
    let rows: Vec<(Vec<u8>, Vec<u8>, Vec<u8>, chrono::NaiveDateTime)> = {
        let base = friend_responses::table
            .inner_join(
                friend_requests::table.on(friend_responses::request_id.eq(friend_requests::id)),
            )
            .filter(friend_responses::responder_hash_id.eq(&req.user_hash))
            .filter(friend_responses::accepted.eq(true))
            .select((
                friend_requests::requester_hash_id,
                friend_requests::pre_key_bundle,
                friend_responses::dilithium_signature,
                friend_responses::created_at,
            ));

        if req.since_timestamp > 0 {
            let since = chrono::DateTime::from_timestamp(req.since_timestamp as i64, 0)
                .map(|dt| dt.naive_utc())
                .unwrap_or_default();
            base.filter(friend_responses::created_at.gt(since))
                .load(&mut conn)
                .map_err(|e| format!("DB error: {}", e))?
        } else {
            base.load(&mut conn)
                .map_err(|e| format!("DB error: {}", e))?
        }
    };

    let responses: Vec<DtoFriendResponse> = rows
        .into_iter()
        .map(|(requester_hash, requester_bundle, our_sig, created_at)| DtoFriendResponse {
            // pre_key_bundle = LEUR bundle (du requester) pour que l'appareil B
            // puisse reconstruire le contact avec leurs vraies clés.
            pre_key_bundle: requester_bundle,
            responder_hash_id: req.user_hash.clone(),
            requester_hash_id: requester_hash,
            accepted: true,
            dilithium_signature: our_sig,
            timestamp: created_at.and_utc().timestamp() as u64,
        })
        .collect();

    Ok(FetchFriendResponsesResponse {
        responses,
        timestamp: current_timestamp(),
    })
}
