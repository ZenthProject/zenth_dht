use diesel::prelude::*;
use chrono::{Utc, Duration};
use rand::{RngCore, thread_rng};
use zenth_dto::{LoginRequest, LoginResponse, AuthChallenge as DtoAuthChallenge, VersionOutdated};
use crate::db::establish_connection;
use crate::models::{User, AuthChallenge, NewAuthChallenge, NewSession};
use crate::schema::{users, auth_challenges, sessions, app_config};
use crate::crypto::verify_dilithium_signature;

fn get_required_version(conn: &mut diesel::PgConnection) -> Option<String> {
    app_config::table
        .filter(app_config::key.eq("required_version"))
        .select(app_config::value)
        .first::<String>(conn)
        .ok()
}

const CHALLENGE_EXPIRY_SECONDS: i64 = 300; // 5 minutes
const SESSION_EXPIRY_HOURS: i64 = 24;

/// Handler pour la méthode LOGIN
/// Flux en 2 étapes:
/// 1. request_challenge=true → retourne un AuthChallenge
/// 2. proof fourni → vérifie et retourne session_token
pub async fn login(req: LoginRequest) -> Result<LoginResponse, String> {
    let mut conn = establish_connection();

    // Vérification de version depuis la DB — pas de restart requis, UPDATE SQL suffit
    if let Some(required) = get_required_version(&mut conn) {
        if req.app_version != required {
            return Ok(LoginResponse {
                success: false,
                challenge: None,
                session_token: vec![],
                session_expiry: 0,
                error_message: String::new(),
                version_outdated: Some(VersionOutdated {
                    min_version: required.clone(),
                    latest_version: required,
                }),
            });
        }
    }

    if req.user_hash_id.is_empty() {
        return Ok(LoginResponse {
            success: false,
            challenge: None,
            session_token: vec![],
            session_expiry: 0,
            error_message: "User hash ID is required".to_string(),
            version_outdated: None,
        });
    }

    if req.user_hash_id.len() != 32 {
        return Ok(LoginResponse {
            success: false,
            challenge: None,
            session_token: vec![],
            session_expiry: 0,
            error_message: format!("Invalid user hash ID length: expected 32, got {}", req.user_hash_id.len()),
            version_outdated: None,
        });
    }

    let user = match users::table
        .filter(users::user_hash_id.eq(&req.user_hash_id))
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::NotFound) => {
            return Ok(LoginResponse {
                success: false,
                challenge: None,
                session_token: vec![],
                session_expiry: 0,
                error_message: "User not found".to_string(),
                version_outdated: None,
            });
        }
        Err(_e) => {
            return Ok(LoginResponse {
                success: false,
                challenge: None,
                session_token: vec![],
                session_expiry: 0,
                error_message: "Database error".to_string(),
                version_outdated: None,
            });
        }
    };

    if req.request_challenge {
        return handle_challenge_request(&mut conn, &req.user_hash_id, user.proof_type);
    }

    if let Some(proof) = req.proof {
        return handle_proof_verification(&mut conn, &user, proof);
    }

    Ok(LoginResponse {
        success: false,
        challenge: None,
        session_token: vec![],
        session_expiry: 0,
        error_message: "Either request_challenge or proof must be provided".to_string(),
        version_outdated: None,
    })
}

/// Génère et stocke un challenge d'authentification
fn handle_challenge_request(
    conn: &mut diesel::PgConnection,
    user_hash_id: &[u8],
    proof_type: i32,
) -> Result<LoginResponse, String> {

    let mut challenge_id = [0u8; 32];
    let mut nonce = [0u8; 32];
    thread_rng().fill_bytes(&mut challenge_id);
    thread_rng().fill_bytes(&mut nonce);

    let now = Utc::now().naive_utc();
    let expires_at = now + Duration::seconds(CHALLENGE_EXPIRY_SECONDS);

    let _ = diesel::delete(
        auth_challenges::table
            .filter(auth_challenges::user_hash_id.eq(user_hash_id))
    ).execute(conn);

    let new_challenge = NewAuthChallenge {
        challenge_id: &challenge_id,
        user_hash_id,
        nonce: &nonce,
        required_proof_type: proof_type,
        public_parameters: &[],
        difficulty: 0,
        expires_at,
    };

    match diesel::insert_into(auth_challenges::table)
        .values(&new_challenge)
        .execute(conn)
    {
        Ok(_) => {
            let timestamp = Utc::now().timestamp() as u64;

            Ok(LoginResponse {
                success: true,
                challenge: Some(DtoAuthChallenge {
                    challenge_id: challenge_id.to_vec(),
                    nonce: nonce.to_vec(),
                    required_proof_type: proof_type,
                    public_parameters: vec![],
                    timestamp,
                    difficulty: 0,
                }),
                session_token: vec![],
                session_expiry: 0,
                error_message: String::new(),
                version_outdated: None,
            })
        }
        Err(_e) => {
            Ok(LoginResponse {
                success: false,
                challenge: None,
                session_token: vec![],
                session_expiry: 0,
                error_message: "Failed to create challenge".to_string(),
                version_outdated: None,
            })
        }
    }
}

/// Vérifie le proof et crée une session
fn handle_proof_verification(
    conn: &mut diesel::PgConnection,
    user: &User,
    proof: zenth_dto::AuthProof,
) -> Result<LoginResponse, String> {

    let challenge = match auth_challenges::table
        .filter(auth_challenges::challenge_id.eq(&proof.challenge_id))
        .filter(auth_challenges::user_hash_id.eq(&user.user_hash_id))
        .select(AuthChallenge::as_select())
        .first(conn)
    {
        Ok(c) => c,
        Err(diesel::NotFound) => {
            return Ok(LoginResponse {
                success: false,
                challenge: None,
                session_token: vec![],
                session_expiry: 0,
                error_message: "Invalid or expired challenge".to_string(),
                version_outdated: None,
            });
        }
        Err(_e) => {
            return Ok(LoginResponse {
                success: false,
                challenge: None,
                session_token: vec![],
                session_expiry: 0,
                error_message: "Database error".to_string(),
                version_outdated: None,
            });
        }
    };

    let now = Utc::now().naive_utc();
    if now > challenge.expires_at {
        let _ = diesel::delete(
            auth_challenges::table.filter(auth_challenges::challenge_id.eq(&proof.challenge_id))
        ).execute(conn);

        return Ok(LoginResponse {
            success: false,
            challenge: None,
            session_token: vec![],
            session_expiry: 0,
            error_message: "Challenge expired".to_string(),
            version_outdated: None,
        });
    }

    let mut message_to_verify = Vec::new();
    message_to_verify.extend_from_slice(&challenge.nonce);
    message_to_verify.extend_from_slice(&user.user_hash_id);
    message_to_verify.extend_from_slice(&proof.timestamp.to_le_bytes());

    let verification_result = verify_dilithium_signature(
        &user.identity_key_dilithium,
        &message_to_verify,
        &proof.proof,
    );

    if !verification_result {
        return Ok(LoginResponse {
            success: false,
            challenge: None,
            session_token: vec![],
            session_expiry: 0,
            error_message: "Invalid proof signature".to_string(),
            version_outdated: None,
        });
    }

    let _ = diesel::delete(
        auth_challenges::table.filter(auth_challenges::challenge_id.eq(&proof.challenge_id))
    ).execute(conn);

    let mut session_token = [0u8; 32];
    thread_rng().fill_bytes(&mut session_token);

    let session_expiry = now + Duration::hours(SESSION_EXPIRY_HOURS);
    let session_expiry_timestamp = (Utc::now() + Duration::hours(SESSION_EXPIRY_HOURS)).timestamp() as u64;

    let new_session = NewSession {
        session_token: &session_token,
        user_hash_id: &user.user_hash_id,
        session_expiry,
    };

    match diesel::insert_into(sessions::table)
        .values(&new_session)
        .execute(conn)
    {
        Ok(_) => {
            Ok(LoginResponse {
                success: true,
                challenge: None,
                session_token: session_token.to_vec(),
                session_expiry: session_expiry_timestamp,
                error_message: String::new(),
                version_outdated: None,
            })
        }
        Err(_e) => {
            Ok(LoginResponse {
                success: false,
                challenge: None,
                session_token: vec![],
                session_expiry: 0,
                error_message: "Failed to create session".to_string(),
                version_outdated: None,
            })
        }
    }
}
