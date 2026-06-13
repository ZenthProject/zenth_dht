use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use std::sync::Arc;
use tokio::sync::broadcast;
use zenth_dto::{
    DhtRequest, DhtResponse, Method,
    WsNotification, NotificationType,
    WsFileAvailableRequest, WsFileChunkRequest, WsFileChunkResponse,
    WsCheckOnlineRequest, WsCheckOnlineResponse,
};

use super::connection_manager::ConnectionManager;
use crate::handlers::decompose::process_request;
use crate::timestamp::current_timestamp;

const USER_HASH_SIZE: usize = 32;

pub async fn handle_websocket(
    socket: WebSocket,
    connection_manager: Arc<ConnectionManager>,
    rustfs_base_url: String,
) {
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
                        let response = process_ws_request(
                            &data,
                            &rustfs_base_url,
                            &user_hash,
                            &connection_manager,
                        ).await;
                        if sender.send(Message::Binary(response.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if sender.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => {}
                }
            }

            broadcast_msg = broadcast_rx.recv() => {
                match broadcast_msg {
                    Ok(msg) => {
                        if sender.send(Message::Binary(msg.payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    connection_manager.unregister(&user_hash).await;
    super::file_relay::unregister_by_sender(&user_hash).await;
}

async fn process_ws_request(
    data: &[u8],
    rustfs_base_url: &str,
    user_hash: &[u8],
    connection_manager: &Arc<ConnectionManager>,
) -> Vec<u8> {
    let request = match DhtRequest::decode(data) {
        Ok(r) => r,
        Err(e) => return encode_error(format!("Décodage DhtRequest échoué: {}", e)),
    };

    let method = Method::try_from(request.method).unwrap_or(Method::Unknown);

    match method {
        Method::WsFileAvailable => {
            ws_file_available(&request.payload, user_hash).await
        }
        Method::WsFileChunkRequest => {
            ws_file_chunk_request(&request.payload, user_hash, connection_manager).await
        }
        Method::WsFileChunk => {
            ws_file_chunk(&request.payload, connection_manager).await
        }
        Method::WsCheckOnline => {
            ws_check_online(&request.payload, &request.request_id, connection_manager).await
        }
        _ => {
            // Traitement HTTP normal via le dispatcher existant
            let response = process_request(request, rustfs_base_url).await;
            let mut buf = Vec::new();
            response.encode(&mut buf).unwrap_or_default();
            buf
        }
    }
}

// ── Handlers WS file transfer ────────────────────────────────────────────────

/// Expéditeur enregistre un transfert disponible.
async fn ws_file_available(payload: &[u8], user_hash: &[u8]) -> Vec<u8> {
    if let Ok(req) = WsFileAvailableRequest::decode(payload) {
        if let Some(tid) = super::file_relay::parse_transfer_id(&req.transfer_id) {
            super::file_relay::register(tid, user_hash.to_vec()).await;
        }
    }
    encode_ok(Method::WsFileAvailable, vec![])
}

/// Destinataire demande un chunk → DHT relay vers l'expéditeur.
async fn ws_file_chunk_request(
    payload: &[u8],
    requester_hash: &[u8],
    connection_manager: &Arc<ConnectionManager>,
) -> Vec<u8> {
    let req = match WsFileChunkRequest::decode(payload) {
        Ok(r) => r,
        Err(_) => return encode_error("payload WsFileChunkRequest invalide".into()),
    };

    let tid = match super::file_relay::parse_transfer_id(&req.transfer_id) {
        Some(t) => t,
        None => return encode_error("transfer_id invalide (doit être 16 bytes)".into()),
    };

    let sender_hash = match super::file_relay::get_sender(&tid).await {
        Some(h) => h,
        None => return encode_error("transfert inconnu ou expéditeur hors ligne".into()),
    };

    // On enrichit la requête avec le hash réel du demandeur (routing retour)
    let forward = WsFileChunkRequest {
        transfer_id: req.transfer_id,
        chunk_index: req.chunk_index,
        requester_hash: requester_hash.to_vec(),
    };

    let notif = build_notification(NotificationType::FileChunkRequest, &forward);
    connection_manager.send_to_user(&sender_hash, notif).await;

    encode_ok(Method::WsFileChunkRequest, vec![])
}

/// Expéditeur envoie un chunk → DHT relay vers le destinataire.
async fn ws_file_chunk(
    payload: &[u8],
    connection_manager: &Arc<ConnectionManager>,
) -> Vec<u8> {
    let chunk = match WsFileChunkResponse::decode(payload) {
        Ok(c) => c,
        Err(_) => return encode_error("payload WsFileChunkResponse invalide".into()),
    };

    if chunk.recipient_hash.is_empty() {
        return encode_error("recipient_hash manquant".into());
    }

    let notif = build_notification(NotificationType::FileChunk, &chunk);
    connection_manager.send_to_user(&chunk.recipient_hash, notif).await;

    encode_ok(Method::WsFileChunk, vec![])
}

/// Vérifie si un utilisateur est connecté via WebSocket.
async fn ws_check_online(
    payload: &[u8],
    request_id: &[u8],
    connection_manager: &Arc<ConnectionManager>,
) -> Vec<u8> {
    let req = match WsCheckOnlineRequest::decode(payload) {
        Ok(r) => r,
        Err(_) => return encode_error("payload WsCheckOnlineRequest invalide".into()),
    };

    let online = connection_manager.is_connected(&req.target_hash).await;
    let resp = WsCheckOnlineResponse { online };

    let mut payload_bytes = Vec::new();
    resp.encode(&mut payload_bytes).unwrap_or_default();

    encode_response(Method::WsCheckOnline, request_id, payload_bytes)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn build_notification(notif_type: NotificationType, msg: &impl ProstMessage) -> Vec<u8> {
    let mut payload = Vec::new();
    msg.encode(&mut payload).unwrap_or_default();

    let notif = WsNotification {
        notification_type: notif_type as i32,
        timestamp: current_timestamp(),
        payload,
    };
    let mut bytes = Vec::new();
    notif.encode(&mut bytes).unwrap_or_default();
    bytes
}

fn encode_ok(method: Method, payload: Vec<u8>) -> Vec<u8> {
    encode_response(method, &[], payload)
}

fn encode_response(method: Method, request_id: &[u8], payload: Vec<u8>) -> Vec<u8> {
    let response = DhtResponse {
        success: true,
        method: method as i32,
        payload,
        error_message: String::new(),
        timestamp: current_timestamp(),
        request_id: request_id.to_vec(),
    };
    let mut buf = Vec::new();
    response.encode(&mut buf).unwrap_or_default();
    buf
}

fn encode_error(msg: String) -> Vec<u8> {
    let response = DhtResponse {
        success: false,
        method: 0,
        payload: vec![],
        error_message: msg,
        timestamp: current_timestamp(),
        request_id: vec![],
    };
    let mut buf = Vec::new();
    response.encode(&mut buf).unwrap_or_default();
    buf
}

// ── Authentification ─────────────────────────────────────────────────────────

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
            if verify_session_token(session_token.to_vec(), user_hash.clone()).await {
                Some(user_hash)
            } else {
                None
            }
        }
        _ => None,
    }
}

async fn verify_session_token(session_token: Vec<u8>, user_hash: Vec<u8>) -> bool {
    tokio::task::spawn_blocking(move || {
        use crate::db::establish_connection;
        use crate::models::Session;
        use diesel::prelude::*;

        let mut conn = establish_connection();
        match crate::schema::sessions::table
            .filter(crate::schema::sessions::session_token.eq(&session_token))
            .filter(crate::schema::sessions::user_hash_id.eq(&user_hash))
            .first::<Session>(&mut conn)
        {
            Ok(session) => session.session_expiry > chrono::Utc::now().naive_utc(),
            Err(_) => false,
        }
    })
    .await
    .unwrap_or(false)
}
