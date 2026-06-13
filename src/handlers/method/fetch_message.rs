use crate::crypto::verify_dilithium_signature;
use crate::db::establish_connection;
use crate::models::User;
use crate::schema::{messages, users};
use diesel::prelude::*;
use prost::Message;
use std::time::{SystemTime, UNIX_EPOCH};
use zenth_dto::zenth_signal_envelope::Content;
use zenth_dto::{FetchMessagesRequest, FetchMessagesResponse, ZenthSignalEnvelope};
use crate::models::MessageModel;

pub fn fetch_messages(
    request: FetchMessagesRequest,
) -> Result<FetchMessagesResponse, Box<dyn std::error::Error>> {
    let mut conn = establish_connection();

    let user = match users::table
        .filter(users::user_hash_id.eq(&request.user_hash))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::result::Error::NotFound) => {
            return Ok(FetchMessagesResponse {
                messages: vec![],
                group_messages: vec![],
                timestamp: 0,
                remaining_count: 0,
            });
        }
        Err(e) => return Err(Box::new(e)),
    };

    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&request.user_hash);
    signed_data.extend_from_slice(&request.since_timestamp.to_le_bytes());
    signed_data.extend_from_slice(&request.limit.to_le_bytes());
    signed_data.extend_from_slice(&request.timestamp.to_le_bytes());

    if !verify_dilithium_signature(
        &user.identity_key_dilithium,
        &signed_data,
        &request.dilithium_signature,
    ) {
        return Ok(FetchMessagesResponse {
            messages: vec![],
            group_messages: vec![],
            timestamp: 0,
            remaining_count: 0,
        });
    }

    let now = chrono::Utc::now().naive_utc();

    let mut query = messages::table
        .filter(messages::recipient_hash_id.eq(&request.user_hash))
        .filter(messages::expires_at.gt(now))
        .order(messages::server_timestamp.asc())
        .into_boxed();

    if request.since_timestamp > 0 {
        query = query.filter(messages::server_timestamp.gt(request.since_timestamp as i64));
    }

    let total_count: i64 = messages::table
        .filter(messages::recipient_hash_id.eq(&request.user_hash))
        .filter(messages::server_timestamp.gt(request.since_timestamp as i64))
        .filter(messages::expires_at.gt(now))
        .count()
        .get_result(&mut conn)?;

    const MAX_LIMIT: u32 = 200;
    const DEFAULT_LIMIT: u32 = 100;
    let limit = match request.limit {
        0 => DEFAULT_LIMIT,
        n => n.min(MAX_LIMIT),
    } as i64;
    query = query.limit(limit);

    let db_messages: Vec<MessageModel> = query.select(MessageModel::as_select()).load(&mut conn)?;

    let envelopes: Vec<ZenthSignalEnvelope> = db_messages
        .into_iter()
        .map(|m| {
            // Try to decode as PrekeyMessage first, then fall back to RegularMessage
            // This is necessary because we store the content as bytes without type info
            let content = if let Ok(prekey_msg) = zenth_dto::PreKeyMessage::decode(m.content.as_slice()) {
                // Check if it looks like a valid PrekeyMessage (has base_key and message)
                if !prekey_msg.base_key.is_empty() || prekey_msg.message.is_some() {
                    Some(Content::PrekeyMessage(prekey_msg))
                } else {
                    // Decode as RegularMessage
                    Some(Content::RegularMessage(
                        prost::Message::decode(m.content.as_slice()).unwrap_or_default(),
                    ))
                }
            } else {
                // Fall back to RegularMessage
                Some(Content::RegularMessage(
                    prost::Message::decode(m.content.as_slice()).unwrap_or_default(),
                ))
            };

            ZenthSignalEnvelope {
                version: 1,
                sender_hash_id: m.sender_hash_id,
                recipient_hash_id: m.recipient_hash_id,
                content,
                dilithium_signature: m.dilithium_signature,
                timestamp: m.timestamp as u64,
                message_id: m.message_id,
                sequence_number: 0,
            }
        })
        .collect();

    let remaining = (total_count - envelopes.len() as i64).max(0) as u32;

    let server_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    Ok(FetchMessagesResponse {
        messages: envelopes,
        group_messages: vec![],
        timestamp: server_timestamp,
        remaining_count: remaining,
    })
}
