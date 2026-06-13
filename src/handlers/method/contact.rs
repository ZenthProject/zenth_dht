use diesel::prelude::*;
use prost::Message;
use zenth_dto::{FriendRequest, FriendRequestNotification};
use crate::db::establish_connection;
use crate::models::{User, NewFriendRequest};
use crate::schema::{users, friend_requests};
use crate::crypto::verify_dilithium_signature;
use crate::websocket::connection_manager::get_global;


pub async fn contact(req: FriendRequest) -> (bool, Vec<u8>, String) {

    if req.requester_hash_id.is_empty() {
        return (false, vec![], "Requester hash ID is required".to_string());
    }

    if req.target_hash_id.is_empty() {
        return (false, vec![], "Target hash ID is required".to_string());
    }

    if req.requester_hash_id.len() != 32 {
        return (false, vec![], format!(
            "Invalid requester hash ID length: expected 32, got {}",
            req.requester_hash_id.len()
        ));
    }

    if req.target_hash_id.len() != 32 {
        return (false, vec![], format!(
            "Invalid target hash ID length: expected 32, got {}",
            req.target_hash_id.len()
        ));
    }

    // Ne pas permettre de s'ajouter soi-même
    if req.requester_hash_id == req.target_hash_id {
        return (false, vec![], "Cannot send friend request to yourself".to_string());
    }

    if req.dilithium_signature.is_empty() {
        return (false, vec![], "Dilithium signature is required".to_string());
    }

    if req.pre_key_bundle.is_empty() {
        return (false, vec![], "Pre-key bundle is required".to_string());
    }

    let mut conn = establish_connection();

    // Vérifier que l'émetteur existe et récupérer sa clé publique
    let requester = match users::table
        .filter(users::user_hash_id.eq(&req.requester_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return (false, vec![], "Requester user not found".to_string());
        }
        Err(_) => {
            return (false, vec![], "Database error".to_string());
        }
    };

    // Vérifier que la cible existe
    let _target = match users::table
        .filter(users::user_hash_id.eq(&req.target_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return (false, vec![], "Target user not found".to_string());
        }
        Err(_) => {
            return (false, vec![], "Database error".to_string());
        }
    };

    let mut message_to_verify = Vec::new();
    message_to_verify.extend_from_slice(&req.target_hash_id);
    message_to_verify.extend_from_slice(&req.pre_key_bundle);
    message_to_verify.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(
        &requester.identity_key_dilithium,
        &message_to_verify,
        &req.dilithium_signature,
    ) {
        return (false, vec![], "Invalid signature".to_string());
    }


    // Vérifier si une demande existe déjà
    let existing = friend_requests::table
        .filter(friend_requests::requester_hash_id.eq(&req.requester_hash_id))
        .filter(friend_requests::target_hash_id.eq(&req.target_hash_id))
        .count()
        .get_result::<i64>(&mut conn);

    if let Ok(count) = existing {
        if count > 0 {
            match diesel::update(
                friend_requests::table
                    .filter(friend_requests::requester_hash_id.eq(&req.requester_hash_id))
                    .filter(friend_requests::target_hash_id.eq(&req.target_hash_id))
            )
            .set((
                friend_requests::pre_key_bundle.eq(&req.pre_key_bundle),
                friend_requests::dilithium_signature.eq(&req.dilithium_signature),
                friend_requests::encrypted_message.eq(if req.encrypted_message.is_empty() {
                    None
                } else {
                    Some(req.encrypted_message.as_slice())
                }),
                friend_requests::timestamp.eq(req.timestamp as i64),
            ))
            .execute(&mut conn)
            {
                Ok(_) => {
                    return (true, vec![], String::new());
                }
                Err(_) => {
                    return (false, vec![], "Failed to update friend request".to_string());
                }
            }
        }
    }

    let new_request = NewFriendRequest {
        requester_hash_id: &req.requester_hash_id,
        target_hash_id: &req.target_hash_id,
        pre_key_bundle: &req.pre_key_bundle,
        dilithium_signature: &req.dilithium_signature,
        encrypted_message: if req.encrypted_message.is_empty() {
            None
        } else {
            Some(&req.encrypted_message)
        },
        timestamp: req.timestamp as i64,
    };

    match diesel::insert_into(friend_requests::table)
        .values(&new_request)
        .execute(&mut conn)
    {
        Ok(_) => {

            notify_friend_request(&req).await;

            (true, vec![], String::new())
        }
        Err(e) => {
            (false, vec![], format!("Failed to store friend request: {}", e))
        }
    }
}

async fn notify_friend_request(req: &FriendRequest) {
    if let Some(manager) = get_global() {
        let notification = FriendRequestNotification {
            requester_hash_id: req.requester_hash_id.clone(),
            pre_key_bundle: req.pre_key_bundle.clone(),
            encrypted_message: req.encrypted_message.clone(),
            timestamp: req.timestamp,
        };

        let ws_notification = zenth_dto::WsNotification {
            notification_type: zenth_dto::NotificationType::FriendRequestReceived as i32,
            timestamp: req.timestamp,
            payload: notification.encode_to_vec(),
        };

        let payload = ws_notification.encode_to_vec();
        manager.send_to_user(&req.target_hash_id, payload).await;
    }
}


