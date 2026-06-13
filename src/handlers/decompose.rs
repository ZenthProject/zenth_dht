use crate::timestamp::current_timestamp;
use axum::{body::Bytes, extract::State, http::StatusCode, response::IntoResponse};
use crate::AppState;
use prost::Message;
use zenth_dto::{
    DhtRequest, DhtResponse, FetchFriendRequestsRequest, FriendRequest, FriendResponse,
    LoginRequest, RegistrationRequest, ZenthSignalEnvelope, FetchMessagesRequest,
    FetchFriendResponsesRequest, LookupUserRequest, FetchPreKeyBundleRequest,
    UploadPreKeysRequest, CheckPreKeyCountRequest, ReplenishPreKeysRequest,
    UpdateManifestRequest, UpdateChunkRequest,
    SyncPushBlobRequest, SyncFetchBlobRequest, SyncDeleteBlobRequest,
    RelayPushRequest, RelayFetchRequest, RelayAckRequest,
    PublishRecoveryKeyRequest, RecoveryClaimRequest,
};
use super::method::delete::DeleteRequest;

use super::method::contact::contact;
use super::method::fetch_friend_requests::fetch_friend_requests;
use super::method::fetch_friend_response::fetch_friend_responses;
use super::method::fetch_message::fetch_messages;
use super::method::fetch_prekey_bundle::fetch_prekey_bundle;
use super::method::login::login;
use super::method::lookup_user::lookup_user;
use super::method::register::register;
use super::method::respond_friend_request::respond_friend_request;
use super::method::delete::delete_account;
use super::method::send_message::send_message;
use super::method::upload_prekeys::upload_prekeys;
use super::method::check_prekey_count::check_prekey_count;
use super::method::replenish_prekeys::replenish_prekeys;
use super::method::update_manifest::get_update_manifest;
use super::method::update_chunk::get_update_chunk;
use super::method::sync_blob::{push_blob, fetch_blob, delete_blob};
use super::method::relay::{push_relay, fetch_relay, ack_relay};
use super::method::fetch_my_accepted::fetch_my_accepted;
use super::method::ack_message::ack_message;
use super::method::recovery::{publish_recovery_key, recovery_claim};

const METHOD_UNKNOWN: i32 = 0;
const METHOD_REGISTER: i32 = 1;
const METHOD_LOGIN: i32 = 2;
const METHOD_DELETE: i32 = 3;
const METHOD_BLOCK: i32 = 4;
const METHOD_GROUP: i32 = 5;
const METHOD_CONTACT: i32 = 6;
const METHOD_REPORT: i32 = 7;
const METHOD_LOOKUP_USER: i32 = 8;
const METHOD_SEND_FRIEND_REQUEST: i32 = 9;
const METHOD_FETCH_FRIEND_REQUESTS: i32 = 10;
const METHOD_RESPOND_FRIEND_REQUEST: i32 = 11;
const METHOD_SEND_MESSAGE: i32 = 12;
const METHOD_FETCH_MESSAGES: i32 = 13;
const METHOD_FETCH_FRIEND_RESPONSES: i32 = 14;
const METHOD_UPLOAD_PREKEYS: i32 = 15;
const METHOD_FETCH_PREKEY_BUNDLE: i32 = 16;
const METHOD_CHECK_PREKEY_COUNT: i32 = 17;
const METHOD_REPLENISH_PREKEYS: i32 = 18;
const METHOD_GET_UPDATE_MANIFEST: i32 = 19;
const METHOD_GET_UPDATE_CHUNK: i32 = 20;
const METHOD_SYNC_PUSH_BLOB: i32 = 21;
const METHOD_SYNC_FETCH_BLOB: i32 = 22;
const METHOD_SYNC_DELETE_BLOB: i32 = 23;
const METHOD_RELAY_PUSH: i32 = 24;
const METHOD_RELAY_FETCH: i32 = 25;
const METHOD_RELAY_ACK: i32 = 26;
const METHOD_FETCH_MY_ACCEPTED: i32 = 27;
const METHOD_ACK_MESSAGE: i32 = 28;
const METHOD_PUBLISH_RECOVERY_KEY: i32 = 29;
const METHOD_RECOVERY_CLAIM: i32 = 30;

pub async fn handle_post(State(state): State<AppState>, body: Bytes) -> impl IntoResponse {
    let dht_request = match DhtRequest::decode(body.as_ref()) {
        Ok(req) => req,
        Err(e) => {
            let response = DhtResponse {
                success: false,
                method: METHOD_UNKNOWN,
                payload: vec![],
                error_message: format!("Error decodage: {}", e),
                timestamp: current_timestamp(),
                request_id: vec![],
            };
            let mut buf = Vec::new();
            response.encode(&mut buf).unwrap();
            return (StatusCode::BAD_REQUEST, buf);
        }
    };

    let dht_response = process_request(dht_request, &state.rustfs_base_url).await;

    let mut buf = Vec::new();
    dht_response.encode(&mut buf).unwrap();

    if dht_response.success {
        (StatusCode::OK, buf)
    } else {
        (StatusCode::BAD_REQUEST, buf)
    }
}

/// Traite une requête DhtRequest et retourne une DhtResponse
/// Utilisé par HTTP POST et WebSocket
pub async fn process_request(dht_request: DhtRequest, rustfs_base_url: &str) -> DhtResponse {

    let (success, response_payload, error_message) = match dht_request.method {
        METHOD_REGISTER => handle_register(&dht_request.payload).await,
        METHOD_LOGIN => handle_login(&dht_request.payload).await,
        METHOD_DELETE => handle_delete(&dht_request.payload).await,
        METHOD_BLOCK => handle_block(&dht_request.payload).await,
        METHOD_GROUP => handle_group(&dht_request.payload).await,
        METHOD_CONTACT => handle_contact(&dht_request.payload).await,
        METHOD_REPORT => handle_report(&dht_request.payload).await,
        METHOD_LOOKUP_USER => handle_lookup_user(&dht_request.payload).await,
        METHOD_SEND_FRIEND_REQUEST => {
            // Same as CONTACT - send a friend request
            handle_contact(&dht_request.payload).await
        }
        METHOD_FETCH_FRIEND_REQUESTS => handle_fetch_friend_requests(&dht_request.payload).await,
        METHOD_RESPOND_FRIEND_REQUEST => handle_respond_friend_request(&dht_request.payload).await,
        METHOD_SEND_MESSAGE => handle_send_message(&dht_request.payload).await,
        METHOD_FETCH_MESSAGES => handle_fetch_messages(&dht_request.payload).await,
        // Friend responses
        METHOD_FETCH_FRIEND_RESPONSES => handle_fetch_friend_responses(&dht_request.payload).await,
        // Pre-keys management
        METHOD_UPLOAD_PREKEYS => handle_upload_prekeys(&dht_request.payload).await,
        METHOD_FETCH_PREKEY_BUNDLE => handle_fetch_prekey_bundle(&dht_request.payload).await,
        METHOD_CHECK_PREKEY_COUNT => handle_check_prekey_count(&dht_request.payload).await,
        METHOD_REPLENISH_PREKEYS => handle_replenish_prekeys(&dht_request.payload).await,
        METHOD_GET_UPDATE_MANIFEST => handle_get_update_manifest(&dht_request.payload, rustfs_base_url).await,
        METHOD_GET_UPDATE_CHUNK => handle_get_update_chunk(&dht_request.payload, rustfs_base_url).await,
        METHOD_SYNC_PUSH_BLOB => handle_sync_push_blob(&dht_request.payload).await,
        METHOD_SYNC_FETCH_BLOB => handle_sync_fetch_blob(&dht_request.payload).await,
        METHOD_SYNC_DELETE_BLOB => handle_sync_delete_blob(&dht_request.payload).await,
        METHOD_RELAY_PUSH => handle_relay_push(&dht_request.payload).await,
        METHOD_RELAY_FETCH => handle_relay_fetch(&dht_request.payload).await,
        METHOD_RELAY_ACK => handle_relay_ack(&dht_request.payload).await,
        METHOD_FETCH_MY_ACCEPTED => handle_fetch_my_accepted(&dht_request.payload).await,
        METHOD_ACK_MESSAGE => handle_ack_message(&dht_request.payload).await,
        METHOD_PUBLISH_RECOVERY_KEY => handle_publish_recovery_key(&dht_request.payload).await,
        METHOD_RECOVERY_CLAIM => handle_recovery_claim(&dht_request.payload).await,
        _ => (false, vec![], "Unknown method".to_string()),
    };

    DhtResponse {
        success,
        method: dht_request.method,
        payload: response_payload,
        error_message,
        timestamp: current_timestamp(),
        request_id: dht_request.request_id,
    }
}

/// Handler pour REGISTER
async fn handle_register(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match RegistrationRequest::decode(payload) {
        Ok(reg_request) => {

            let result = register(reg_request).await;

            match result {
                Ok(response) => {
                    let success = response.success;
                    let mut buf = Vec::new();
                    response.encode(&mut buf).unwrap();
                    (success, buf, String::new())
                }
                Err(e) => (false, vec![], e),
            }
        }
        Err(e) => (
            false,
            vec![],
            format!("Erreur décodage RegistrationRequest: {}", e),
        ),
    }
}

/// Handler for LOGIN
async fn handle_login(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match LoginRequest::decode(payload) {
        Ok(login_request) => {
            let result = login(login_request).await;

            match result {
                Ok(response) => {
                    let success = response.success;
                    let mut buf = Vec::new();
                    response.encode(&mut buf).unwrap();
                    (success, buf, String::new())
                }
                Err(e) => (false, vec![], e),
            }
        }
        Err(e) => (
            false,
            vec![],
            format!("Erreur décodage LoginRequest: {}", e),
        ),
    }
}

async fn handle_delete(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match DeleteRequest::decode(payload) {
        Ok(req) => match delete_account(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding DeleteRequest: {}", e)),
    }
}

async fn handle_block(_payload: &[u8]) -> (bool, Vec<u8>, String) {
    (false, vec![], "BLOCK non implémenté".to_string())
}

async fn handle_group(_payload: &[u8]) -> (bool, Vec<u8>, String) {
    (false, vec![], "GROUP non implémenté".to_string())
}

async fn handle_contact(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match FriendRequest::decode(payload) {
        Ok(friend_request) => contact(friend_request).await,
        Err(e) => (
            false,
            vec![],
            format!("Error decoding FriendRequest: {}", e),
        ),
    }
}

/// Handler pour REPORT
async fn handle_report(_payload: &[u8]) -> (bool, Vec<u8>, String) {
    // TODO: Implémenter
    (false, vec![], "REPORT non implémenté".to_string())
}

/// Handler pour LOOKUP_USER (METHOD 8)
async fn handle_lookup_user(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match LookupUserRequest::decode(payload) {
        Ok(request) => match lookup_user(request).await {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (
            false,
            vec![],
            format!("Error decoding LookupUserRequest: {}", e),
        ),
    }
}

/// Handler pour FETCH_FRIEND_REQUESTS (METHOD 10)
async fn handle_fetch_friend_requests(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match FetchFriendRequestsRequest::decode(payload) {
        Ok(request) => match fetch_friend_requests(request).await {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (
            false,
            vec![],
            format!("Error decoding FetchFriendRequestsRequest: {}", e),
        ),
    }
}

/// Handler pour RESPOND_FRIEND_REQUEST (METHOD 11)
async fn handle_respond_friend_request(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match FriendResponse::decode(payload) {
        Ok(request) => respond_friend_request(request).await,
        Err(e) => (
            false,
            vec![],
            format!("Error decoding FriendResponse: {}", e),
        ),
    }
}

async fn handle_send_message(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match ZenthSignalEnvelope::decode(payload) {
        Ok(envelope) => match send_message(envelope) {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, String::new())
            }
            Err(e) => (false, vec![], format!("Send message error: {}", e)),
        },
        Err(e) => (
            false,
            vec![],
            format!("Error decoding ZenthSignalEnvelope: {}", e),
        ),
    }
}

async fn handle_fetch_messages(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match FetchMessagesRequest::decode(payload) {
        Ok(request) => match fetch_messages(request) {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], format!("Fetch messages error: {}", e)),
        },
        Err(e) => (
            false,
            vec![],
            format!("Error decoding FetchMessagesRequest: {}", e),
        ),
    }
}

async fn handle_fetch_friend_responses(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match FetchFriendResponsesRequest::decode(payload) {
        Ok(request) => match fetch_friend_responses(request).await {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (
            false,
            vec![],
            format!("Error decoding FetchFriendResponsesRequest: {}", e),
        ),
    }
}

/// Handler pour UPLOAD_PREKEYS (METHOD 15)
async fn handle_upload_prekeys(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match UploadPreKeysRequest::decode(payload) {
        Ok(req) => match upload_prekeys(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding UploadPreKeysRequest: {}", e)),
    }
}

/// Handler pour FETCH_PREKEY_BUNDLE (METHOD 16)
async fn handle_fetch_prekey_bundle(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match FetchPreKeyBundleRequest::decode(payload) {
        Ok(request) => match fetch_prekey_bundle(request).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (
            false,
            vec![],
            format!("Error decoding FetchPreKeyBundleRequest: {}", e),
        ),
    }
}

/// Handler pour CHECK_PREKEY_COUNT (METHOD 17)
async fn handle_check_prekey_count(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match CheckPreKeyCountRequest::decode(payload) {
        Ok(req) => match check_prekey_count(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding CheckPreKeyCountRequest: {}", e)),
    }
}

/// Handler pour REPLENISH_PREKEYS (METHOD 18)
async fn handle_replenish_prekeys(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match ReplenishPreKeysRequest::decode(payload) {
        Ok(req) => match replenish_prekeys(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding ReplenishPreKeysRequest: {}", e)),
    }
}

/// Handler pour GET_UPDATE_MANIFEST (METHOD 19)
async fn handle_get_update_manifest(payload: &[u8], rustfs_base_url: &str) -> (bool, Vec<u8>, String) {
    match UpdateManifestRequest::decode(payload) {
        Ok(req) => match get_update_manifest(req, rustfs_base_url).await {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding UpdateManifestRequest: {}", e)),
    }
}

/// Handler pour GET_UPDATE_CHUNK (METHOD 20)
async fn handle_get_update_chunk(payload: &[u8], rustfs_base_url: &str) -> (bool, Vec<u8>, String) {
    match UpdateChunkRequest::decode(payload) {
        Ok(req) => match get_update_chunk(req, rustfs_base_url).await {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding UpdateChunkRequest: {}", e)),
    }
}

async fn handle_sync_push_blob(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match SyncPushBlobRequest::decode(payload) {
        Ok(req) => match push_blob(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding SyncPushBlobRequest: {}", e)),
    }
}

async fn handle_sync_fetch_blob(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match SyncFetchBlobRequest::decode(payload) {
        Ok(req) => match fetch_blob(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding SyncFetchBlobRequest: {}", e)),
    }
}

async fn handle_sync_delete_blob(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match SyncDeleteBlobRequest::decode(payload) {
        Ok(req) => match delete_blob(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding SyncDeleteBlobRequest: {}", e)),
    }
}

async fn handle_relay_push(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match RelayPushRequest::decode(payload) {
        Ok(req) => match push_relay(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding RelayPushRequest: {}", e)),
    }
}

async fn handle_relay_fetch(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match RelayFetchRequest::decode(payload) {
        Ok(req) => match fetch_relay(req).await {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding RelayFetchRequest: {}", e)),
    }
}

async fn handle_relay_ack(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match RelayAckRequest::decode(payload) {
        Ok(req) => match ack_relay(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding RelayAckRequest: {}", e)),
    }
}

async fn handle_ack_message(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match zenth_dto::MessageAck::decode(payload) {
        Ok(req) => match ack_message(req) {
            Ok(()) => (true, vec![], String::new()),
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding MessageAck: {}", e)),
    }
}

async fn handle_fetch_my_accepted(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match FetchFriendResponsesRequest::decode(payload) {
        Ok(req) => match fetch_my_accepted(req).await {
            Ok(response) => {
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (true, buf, String::new())
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding FetchFriendResponsesRequest: {}", e)),
    }
}

async fn handle_publish_recovery_key(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match PublishRecoveryKeyRequest::decode(payload) {
        Ok(req) => match publish_recovery_key(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding PublishRecoveryKeyRequest: {}", e)),
    }
}

async fn handle_recovery_claim(payload: &[u8]) -> (bool, Vec<u8>, String) {
    match RecoveryClaimRequest::decode(payload) {
        Ok(req) => match recovery_claim(req).await {
            Ok(response) => {
                let success = response.success;
                let mut buf = Vec::new();
                response.encode(&mut buf).unwrap();
                (success, buf, if success { String::new() } else { response.error_message })
            }
            Err(e) => (false, vec![], e),
        },
        Err(e) => (false, vec![], format!("Error decoding RecoveryClaimRequest: {}", e)),
    }
}
