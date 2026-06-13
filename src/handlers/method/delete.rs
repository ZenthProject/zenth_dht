use diesel::prelude::*;
use crate::db::establish_connection;

#[derive(Clone, prost::Message)]
pub struct DeleteRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub user_hash_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub dilithium_signature: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub timestamp: u64,
}

#[derive(Clone, prost::Message)]
pub struct DeleteResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(string, tag = "2")]
    pub error_message: String,
}
use crate::models::User;
use crate::schema::{users, sessions, auth_challenges, friend_requests, friend_responses, messages};
use crate::crypto::verify_dilithium_signature;
use crate::websocket::connection_manager::get_global;

/// Supprime le compte d'un utilisateur après vérification Dilithium.
/// Appelée lors d'une demande explicite de l'utilisateur.
pub async fn delete_account(req: DeleteRequest) -> Result<DeleteResponse, String> {
    if req.user_hash_id.is_empty() || req.user_hash_id.len() != 32 {
        return Ok(DeleteResponse {
            success: false,
            error_message: "Invalid user hash".to_string(),
        });
    }

    if req.dilithium_signature.is_empty() {
        return Ok(DeleteResponse {
            success: false,
            error_message: "Signature required".to_string(),
        });
    }

    if !crate::timestamp::is_timestamp_fresh(req.timestamp) {
        return Ok(DeleteResponse {
            success: false,
            error_message: "Request timestamp expired".to_string(),
        });
    }

    let mut conn = establish_connection();

    let user = match users::table
        .filter(users::user_hash_id.eq(&req.user_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Ok(DeleteResponse {
                success: false,
                error_message: "User not found".to_string(),
            });
        }
        Err(_) => {
            return Ok(DeleteResponse {
                success: false,
                error_message: "Database error".to_string(),
            });
        }
    };

    // Vérifie sign(user_hash_id || timestamp)
    let mut msg = Vec::new();
    msg.extend_from_slice(&req.user_hash_id);
    msg.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&user.identity_key_dilithium, &msg, &req.dilithium_signature) {
        return Ok(DeleteResponse {
            success: false,
            error_message: "Invalid signature".to_string(),
        });
    }

    if let Err(e) = delete_user_data(&req.user_hash_id, &mut conn) {
        return Ok(DeleteResponse { success: false, error_message: e });
    }

    // Déconnecte la session WebSocket active
    if let Some(manager) = get_global() {
        manager.unregister(&req.user_hash_id).await;
    }

    Ok(DeleteResponse { success: true, error_message: String::new() })
}

pub fn delete_user_data(user_hash_id: &[u8], conn: &mut PgConnection) -> Result<(), String> {
    // 1. friend_responses liés aux demandes impliquant cet utilisateur
    let request_ids: Vec<i32> = friend_requests::table
        .filter(
            friend_requests::requester_hash_id.eq(user_hash_id)
                .or(friend_requests::target_hash_id.eq(user_hash_id)),
        )
        .select(friend_requests::id)
        .load(conn)
        .map_err(|e| format!("Failed to load friend request ids: {}", e))?;

    if !request_ids.is_empty() {
        diesel::delete(
            friend_responses::table.filter(friend_responses::request_id.eq_any(&request_ids)),
        )
        .execute(conn)
        .map_err(|e| format!("Failed to delete friend responses: {}", e))?;
    }

    // 2. friend_requests
    diesel::delete(
        friend_requests::table.filter(
            friend_requests::requester_hash_id.eq(user_hash_id)
                .or(friend_requests::target_hash_id.eq(user_hash_id)),
        ),
    )
    .execute(conn)
    .map_err(|e| format!("Failed to delete friend requests: {}", e))?;

    // 3. messages (envoyés et reçus)
    diesel::delete(
        messages::table.filter(
            messages::sender_hash_id.eq(user_hash_id)
                .or(messages::recipient_hash_id.eq(user_hash_id)),
        ),
    )
    .execute(conn)
    .map_err(|e| format!("Failed to delete messages: {}", e))?;

    // 4. sessions
    diesel::delete(sessions::table.filter(sessions::user_hash_id.eq(user_hash_id)))
        .execute(conn)
        .map_err(|e| format!("Failed to delete sessions: {}", e))?;

    // 5. auth challenges
    diesel::delete(auth_challenges::table.filter(auth_challenges::user_hash_id.eq(user_hash_id)))
        .execute(conn)
        .map_err(|e| format!("Failed to delete auth challenges: {}", e))?;

    // 6. utilisateur
    diesel::delete(users::table.filter(users::user_hash_id.eq(user_hash_id)))
        .execute(conn)
        .map_err(|e| format!("Failed to delete user: {}", e))?;

    Ok(())
}
