use crate::db::establish_connection;
use crate::models::{User, NewMessage};
use crate::schema::{users, messages};
use crate::websocket::get_global;
use diesel::prelude::*;
use zenth_dto::{SendMessageResponse, ZenthSignalEnvelope, WsNotification, MessageNotification, NotificationType};
use zenth_dto::zenth_signal_envelope::Content;
use prost::Message;
use crate::crypto::verify_dilithium_signature;
use std::time::{SystemTime, UNIX_EPOCH};                                                                                                                      



pub fn send_message(
    envelope: ZenthSignalEnvelope,
) -> Result<SendMessageResponse, Box<dyn std::error::Error>> {
    let mut conn = establish_connection();

    // 1. Vérifie que le sender existe
    let _sender = match users::table
        .filter(users::user_hash_id.eq(&envelope.sender_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::result::Error::NotFound) => {
            return Ok(SendMessageResponse {
                success: false,
                message_id: vec![],
                server_timestamp: 0,
                error_message: "Sender not found".to_string(),
            });
        }
        Err(_) => {
            return Ok(SendMessageResponse {
                success: false,
                message_id: vec![],
                server_timestamp: 0,
                error_message: "Database error".to_string(),
            });
        }
    };

    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&envelope.sender_hash_id);
    signed_data.extend_from_slice(&envelope.recipient_hash_id);
    signed_data.extend_from_slice(&envelope.message_id);
    signed_data.extend_from_slice(&envelope.timestamp.to_le_bytes());
    if let Some(ref content) = envelope.content {                                                                                                                 
        match content {                                                                                                                                           
            Content::PrekeyMessage(msg) => {                                                                                                                      
            signed_data.extend_from_slice(&msg.encode_to_vec());                                                                                              
            }                                                                                                                                                     
            Content::RegularMessage(msg) => {                                                                                                                     
                signed_data.extend_from_slice(&msg.encode_to_vec());                                                                                              
            }                                                                                                                                                     
        }                                                                                                                                                         
    }     

    if !verify_dilithium_signature(
        &_sender.identity_key_dilithium,
        &signed_data,
        &envelope.dilithium_signature,
    ) {
        return Ok(SendMessageResponse {
            success: false,
            message_id: vec![],
            server_timestamp: 0,
            error_message: "Invalid signature".to_string(),
        });
    }

    // Verifier que le recipient existe
    let _recipient = match users::table
        .filter(users::user_hash_id.eq(&envelope.recipient_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::result::Error::NotFound) => {
            return Ok(SendMessageResponse {
                success: false,
                message_id: vec![],
                server_timestamp: 0,
                error_message: "Recipient not found".to_string(),
            });
        }
        Err(_) => {
            return Ok(SendMessageResponse {
                success: false,
                message_id: vec![],
                server_timestamp: 0,
                error_message: "Database error".to_string(),
            });
        }
    };
    // TODO: 4. Stocker le message en BDD (table messages)


          let server_timestamp = SystemTime::now()                                                                                                                  
          .duration_since(UNIX_EPOCH)                                                                                                                           
          .unwrap()                                                                                                                                             
          .as_millis() as i64;                                                                                                                                  
                                                                                                                                                                
    // Sérialiser le content
    let content_bytes = match &envelope.content {
        Some(content) => match content {
            Content::PrekeyMessage(msg) => msg.encode_to_vec(),
            Content::RegularMessage(msg) => msg.encode_to_vec(),
        },
        None => vec![],
    };

    // Rate limit par palier de taille (le serveur voit les octets chiffrés, pas le contenu).
    const MID_THRESHOLD:   usize = 1024 * 1024;       // 1 Mo  — en dessous : pas de limite
    const LARGE_THRESHOLD: usize = 10 * 1024 * 1024;  // 10 Mo — palier moyen
    const MAX_THRESHOLD:   usize = 25 * 1024 * 1024;  // 25 Mo — refusé au-dessus

    let size = content_bytes.len();
    if size > MAX_THRESHOLD {
        return Ok(SendMessageResponse {
            success: false,
            message_id: vec![],
            server_timestamp: 0,
            error_message: "Fichier trop volumineux : maximum 25 Mo par message (utilisez le transfert P2P pour les fichiers plus grands)".to_string(),
        });
    }
    if size > LARGE_THRESHOLD && !crate::rate_limit::large_msg_allowed(&envelope.sender_hash_id) {
        return Ok(SendMessageResponse {
            success: false,
            message_id: vec![],
            server_timestamp: 0,
            error_message: "Limite dépassée : 1 fichier > 10 Mo autorisé par 30 secondes".to_string(),
        });
    }
    if size > MID_THRESHOLD && !crate::rate_limit::mid_msg_allowed(&envelope.sender_hash_id) {
        return Ok(SendMessageResponse {
            success: false,
            message_id: vec![],
            server_timestamp: 0,
            error_message: "Limite dépassée : 1 fichier > 1 Mo autorisé par 10 secondes".to_string(),
        });
    }

    // sequence_number transporte le TTL souhaité par l'expéditeur (0 = jamais).
    let expires_at = if envelope.sequence_number == 0 {
        // Pas d'expiration : on utilise une date très lointaine (year 9999).
        chrono::NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(9999, 12, 31).unwrap(),
            chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
        )
    } else {
        chrono::Utc::now().naive_utc()
            + chrono::Duration::hours(envelope.sequence_number as i64)
    };

    let new_message = NewMessage {
        message_id: &envelope.message_id,
        sender_hash_id: &envelope.sender_hash_id,
        recipient_hash_id: &envelope.recipient_hash_id,
        content: &content_bytes,
        dilithium_signature: &envelope.dilithium_signature,
        timestamp: envelope.timestamp as i64,
        server_timestamp,
        expires_at,
    };                                                                                                                                                        
                                                                                                                                                                
    diesel::insert_into(messages::table)
        .values(&new_message)
        .execute(&mut conn)?;

    // Push le message au destinataire s'il est connecté via WebSocket
    push_to_recipient(&envelope);

    Ok(SendMessageResponse {
        success: true,
        message_id: envelope.message_id,
        server_timestamp: server_timestamp as u64,
        error_message: String::new(),
    })
}

/// Push un message au destinataire via WebSocket s'il est connecté
fn push_to_recipient(envelope: &ZenthSignalEnvelope) {
    // Récupérer le ConnectionManager global
    let Some(manager) = get_global() else {
        return;
    };

    // Créer MessageNotification (notification légère, pas l'envelope complet)
    let msg_notification = MessageNotification {
        sender_hash_id: envelope.sender_hash_id.clone(),
        message_id: envelope.message_id.clone(),
        timestamp: envelope.timestamp,
    };

    // Encoder MessageNotification comme payload
    let mut notification_payload = Vec::new();
    if msg_notification.encode(&mut notification_payload).is_err() {
        return;
    }

    // Créer WsNotification avec le bon type
    let ws_notification = WsNotification {
        notification_type: NotificationType::MessageReceived as i32,
        timestamp: crate::timestamp::current_timestamp(),
        payload: notification_payload,
    };

    let mut response_bytes = Vec::new();
    if ws_notification.encode(&mut response_bytes).is_err() {
        return;
    }

    let recipient_hash = envelope.recipient_hash_id.clone();
    tokio::spawn(async move {
        manager.send_to_user(&recipient_hash, response_bytes).await;
    });
}

