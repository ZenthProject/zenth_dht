use diesel::prelude::*;
use zenth_dto::{RegistrationRequest, RegistrationResponse};
use crate::db::establish_connection;
use crate::models::NewUser;
use crate::schema::users;

pub async fn register(req: RegistrationRequest) -> Result<RegistrationResponse, String> {

    if req.username_hash.is_empty() {
        return Err("Username hash vide".to_string());
    }

    if req.username_hash.len() != 32 {
        return Err(format!("Username hash invalide: attendu 32 bytes, reçu {}", req.username_hash.len()));
    }

    if req.pre_key_bundle.is_empty() {
        return Err("Pre-key bundle vide".to_string());
    }

    if req.password_commitment.is_empty() {
        return Err("Password commitment vide".to_string());
    }

    if req.identity_key_dilithium.is_empty() {
        return Err("Identity key vide".to_string());
    }

    if req.identity_signature.is_empty() {
        return Err("Identity signature vide".to_string());
    }

    // Créer le nouvel utilisateur
    let new_user = NewUser {
        user_hash_id: &req.username_hash,
        password_commitment: &req.password_commitment,
        identity_key_dilithium: &req.identity_key_dilithium,
        identity_signature: &req.identity_signature,
        pre_key_bundle: &req.pre_key_bundle,
        proof_type: req.proof_type,
        recovery_dilithium_pubkey: None,
    };

    // Sauvegarder en base de données
    let mut conn = establish_connection();

    match diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut conn)
    {
        Ok(_) => {
            Ok(RegistrationResponse {
                success: true,
                user_hash_id: req.username_hash,
                challenge_parameters: vec![],
                error_message: String::new(),
            })
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("duplicate key") || error_msg.contains("unique constraint") {
                Ok(RegistrationResponse {
                    success: false,
                    user_hash_id: vec![],
                    challenge_parameters: vec![],
                    error_message: "User already exists".to_string(),
                })
            } else {
                Ok(RegistrationResponse {
                    success: false,
                    user_hash_id: vec![],
                    challenge_parameters: vec![],
                    error_message: format!("Database error: {}", e),
                })
            }
        }
    }
}
