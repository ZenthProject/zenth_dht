use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::User;
use crate::schema::{messages, users};
use diesel::prelude::*;
use zenth_dto::MessageAck;

/// Accusé de réception d'un message : supprime immédiatement le message du DHT.
///
/// Le client envoie cet ack après avoir stocké le message localement.
/// Authentification : signature Dilithium sur message_id || recipient_hash_id || timestamp.
pub fn ack_message(req: MessageAck) -> Result<(), String> {
    if req.message_id.is_empty() || req.recipient_hash_id.is_empty() {
        return Err("message_id et recipient_hash_id requis".to_string());
    }
    if req.dilithium_signature.is_empty() {
        return Err("Signature Dilithium requise".to_string());
    }

    let mut conn = establish_connection();

    // Vérifie que le destinataire existe et récupère sa clé publique
    let user = match users::table
        .filter(users::user_hash_id.eq(&req.recipient_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => return Err("Destinataire introuvable".to_string()),
        Err(e) => return Err(format!("DB error: {}", e)),
    };

    // Vérifie la signature : message_id || recipient_hash_id || timestamp
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&req.message_id);
    signed_data.extend_from_slice(&req.recipient_hash_id);
    signed_data.extend_from_slice(&req.timestamp.to_le_bytes());

    if !verify_dilithium_signature(&user.identity_key_dilithium, &signed_data, &req.dilithium_signature) {
        return Err("Signature invalide".to_string());
    }

    // Suppression : uniquement si recipient correspond (évite qu'un tiers supprime les messages)
    diesel::delete(
        messages::table
            .filter(messages::message_id.eq(&req.message_id))
            .filter(messages::recipient_hash_id.eq(&req.recipient_hash_id)),
    )
    .execute(&mut conn)
    .map_err(|e| format!("Suppression échouée: {}", e))?;

    Ok(())
}
