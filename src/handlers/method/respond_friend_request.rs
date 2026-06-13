use diesel::prelude::*;
use prost::Message;
use zenth_dto::{FriendResponse, WsNotification, NotificationType};
use crate::db::establish_connection;
use crate::models::{User, FriendRequestModel, NewFriendResponse};
use crate::schema::{users, friend_requests, friend_responses};
use crate::crypto::verify_dilithium_signature;
use crate::websocket::connection_manager::get_global;

pub async fn respond_friend_request(req: FriendResponse) -> (bool, Vec<u8>, String) {

    if req.responder_hash_id.is_empty() {
        return (false, vec![], "Responder hash ID is required".to_string());
    }

    if req.requester_hash_id.is_empty() {
        return (false, vec![], "Requester hash ID is required".to_string());
    }

    if req.responder_hash_id.len() != 32 {
        return (false, vec![], format!(
            "Invalid responder hash ID length: expected 32, got {}",
            req.responder_hash_id.len()
        ));
    }

    if req.requester_hash_id.len() != 32 {
        return (false, vec![], format!(
            "Invalid requester hash ID length: expected 32, got {}",
            req.requester_hash_id.len()
        ));
    }

    if req.dilithium_signature.is_empty() {
        return (false, vec![], "Dilithium signature is required".to_string());
    }

    // Si accepté, le pre_key_bundle est requis
    if req.accepted && req.pre_key_bundle.is_empty() {
        return (false, vec![], "Pre-key bundle is required when accepting".to_string());
    }

    let mut conn = establish_connection();

    // Vérifier que le responder existe et récupérer sa clé publique
    let responder = match users::table
        .filter(users::user_hash_id.eq(&req.responder_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return (false, vec![], "Responder user not found".to_string());
        }
        Err(_) => {
            return (false, vec![], "Database error".to_string());
        }
    };

    let message_to_verify = if req.accepted {
        let mut msg = Vec::new();
        msg.extend_from_slice(b"FRIENDSHIP:");
        msg.extend_from_slice(&req.responder_hash_id);
        msg.extend_from_slice(&req.requester_hash_id);
        msg
    } else {
        let mut msg = Vec::new();
        msg.extend_from_slice(&req.requester_hash_id);
        msg.push(0u8);
        msg.extend_from_slice(&req.timestamp.to_le_bytes());
        msg
    };

    if !verify_dilithium_signature(
        &responder.identity_key_dilithium,
        &message_to_verify,
        &req.dilithium_signature,
    ) {
        return (false, vec![], "Invalid signature".to_string());
    }


    let original_request = match friend_requests::table
        .filter(friend_requests::requester_hash_id.eq(&req.requester_hash_id))
        .filter(friend_requests::target_hash_id.eq(&req.responder_hash_id))
        .select(FriendRequestModel::as_select())
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(diesel::NotFound) => {
            return (false, vec![], "Friend request not found".to_string());
        }
        Err(_) => {
            return (false, vec![], "Database error".to_string());
        }
    };

    // Vérifier si une réponse existe déjà
    let existing_response = friend_responses::table
        .filter(friend_responses::request_id.eq(original_request.id))
        .count()
        .get_result::<i64>(&mut conn);

    if let Ok(count) = existing_response {
        if count > 0 {
            match diesel::update(
                friend_responses::table.filter(friend_responses::request_id.eq(original_request.id))
            )
            .set((
                friend_responses::accepted.eq(req.accepted),
                friend_responses::pre_key_bundle.eq(if req.pre_key_bundle.is_empty() {
                    None
                } else {
                    Some(req.pre_key_bundle.as_slice())
                }),
                friend_responses::dilithium_signature.eq(&req.dilithium_signature),
                friend_responses::delivered.eq(false), // Reset delivered flag
            ))
            .execute(&mut conn)
            {
                Ok(_) => {
                    return (true, vec![], String::new());
                }
                Err(_) => {
                    return (false, vec![], "Failed to update response".to_string());
                }
            }
        }
    }

    // Créer la nouvelle réponse
    let new_response = NewFriendResponse {
        request_id: original_request.id,
        responder_hash_id: &req.responder_hash_id,
        requester_hash_id: &req.requester_hash_id,
        accepted: req.accepted,
        pre_key_bundle: if req.pre_key_bundle.is_empty() {
            None
        } else {
            Some(&req.pre_key_bundle)
        },
        dilithium_signature: &req.dilithium_signature,
    };

    match diesel::insert_into(friend_responses::table)
        .values(&new_response)
        .execute(&mut conn)
    {
        Ok(_) => {
            notify_friend_response(&req).await;

            (true, vec![], String::new())
        }
        Err(e) => {
            (false, vec![], format!("Failed to store response: {}", e))
        }
    }
}

/// Send a WebSocket notification to the original requester about the response
async fn notify_friend_response(req: &FriendResponse) {
    if let Some(manager) = get_global() {
        // Determine notification type based on accepted flag
        let notification_type = if req.accepted {
            NotificationType::FriendRequestAccepted
        } else {
            NotificationType::FriendRequestRejected
        };

        let ws_notification = WsNotification {
            notification_type: notification_type as i32,
            timestamp: req.timestamp,
            payload: req.encode_to_vec(),
        };

        let payload = ws_notification.encode_to_vec();
        manager.send_to_user(&req.requester_hash_id, payload).await;
    }
}

