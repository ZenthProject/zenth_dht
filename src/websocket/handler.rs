use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use std::sync::Arc;
use tokio::sync::broadcast;
use zenth_dto::{DhtRequest, DhtResponse};

use super::connection_manager::ConnectionManager;
use crate::handlers::decompose::process_request;
use crate::timestamp::current_timestamp;

const USER_HASH_SIZE: usize = 32;

pub async fn handle_websocket(socket: WebSocket, connection_manager: Arc<ConnectionManager>, rustfs_base_url: String) {
    let (mut sender, mut receiver) = socket.split();

    let user_hash = match authenticate(&mut receiver).await {
        Some(hash) => hash,
        None => {
            let _ = sender.close().await;
            return;
        }
    };

    let mut broadcast_rx = connection_manager.register(user_hash.clone()).await;

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        let response = process_websocket_request(&data, &rustfs_base_url).await;
                        if let Err(_) = sender.send(Message::Binary(response.into())).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if let Err(_) = sender.send(Message::Pong(data)).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        break;
                    }
                    Some(Err(_)) => {
                        break;
                    }
                    None => {
                        break;
                    }
                    _ => {}
                }
            }

            broadcast_msg = broadcast_rx.recv() => {
                match broadcast_msg {
                    Ok(msg) => {
                        if let Err(_) = sender.send(Message::Binary(msg.payload.into())).await {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    connection_manager.unregister(&user_hash).await;
}

async fn authenticate(
    receiver: &mut futures::stream::SplitStream<WebSocket>,
) -> Option<Vec<u8>> {
    let auth_timeout = tokio::time::Duration::from_secs(10);

    match tokio::time::timeout(auth_timeout, receiver.next()).await {
        Ok(Some(Ok(Message::Binary(data)))) => {
            if data.len() <= USER_HASH_SIZE {
                return None;
            }

            let user_hash = data[..USER_HASH_SIZE].to_vec();
            let session_token = &data[USER_HASH_SIZE..];

            if verify_session_token(session_token, &user_hash) {
                Some(user_hash)
            } else {
                None
            }
        }
        Ok(_) => {
            None
        }
        Err(_) => {
            None
        }
    }
}

fn verify_session_token(session_token: &[u8], user_hash: &[u8]) -> bool {
    use crate::db::establish_connection;
    use crate::models::Session;
    use diesel::prelude::*;

    let mut conn = establish_connection();

    let result = crate::schema::sessions::table
        .filter(crate::schema::sessions::session_token.eq(session_token))
        .filter(crate::schema::sessions::user_hash_id.eq(user_hash))
        .first::<Session>(&mut conn);

    match result {
        Ok(session) => {
            let now = chrono::Utc::now().naive_utc();
            session.session_expiry > now
        }
        Err(_) => false,
    }
}

async fn process_websocket_request(data: &[u8], rustfs_base_url: &str) -> Vec<u8> {
    match DhtRequest::decode(data) {
        Ok(request) => {
            let response = process_request(request, rustfs_base_url).await;
            let mut buf = Vec::new();
            response.encode(&mut buf).unwrap();
            buf
        }
        Err(e) => {
            let response = DhtResponse {
                success: false,
                method: 0,
                payload: vec![],
                error_message: format!("Erreur décodage: {}", e),
                timestamp: current_timestamp(),
                request_id: vec![],
            };
            let mut buf = Vec::new();
            response.encode(&mut buf).unwrap();
            buf
        }
    }
}
