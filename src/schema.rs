// @generated automatically by Diesel CLI.

diesel::table! {
    app_config (key) {
        key -> Varchar,
        value -> Varchar,
    }
}

diesel::table! {
    auth_challenges (challenge_id) {
        challenge_id -> Bytea,
        user_hash_id -> Bytea,
        nonce -> Bytea,
        required_proof_type -> Int4,
        public_parameters -> Bytea,
        difficulty -> Int4,
        created_at -> Timestamp,
        expires_at -> Timestamp,
    }
}

diesel::table! {
    friend_requests (id) {
        id -> Int4,
        requester_hash_id -> Bytea,
        target_hash_id -> Bytea,
        pre_key_bundle -> Bytea,
        dilithium_signature -> Bytea,
        encrypted_message -> Nullable<Bytea>,
        timestamp -> Int8,
        created_at -> Timestamp,
    }
}

diesel::table! {
    friend_responses (id) {
        id -> Int4,
        request_id -> Int4,
        responder_hash_id -> Bytea,
        requester_hash_id -> Bytea,
        accepted -> Bool,
        pre_key_bundle -> Nullable<Bytea>,
        dilithium_signature -> Bytea,
        delivered -> Bool,
        created_at -> Timestamp,
    }
}

diesel::table! {
    messages (id) {
        id -> Int4,
        message_id -> Bytea,
        sender_hash_id -> Bytea,
        recipient_hash_id -> Bytea,
        content -> Bytea,
        dilithium_signature -> Bytea,
        timestamp -> Int8,
        server_timestamp -> Int8,
        delivered -> Bool,
        created_at -> Timestamp,
        expires_at -> Timestamp,
    }
}

diesel::table! {
    one_time_prekeys (id) {
        id -> Int4,
        user_hash_id -> Bytea,
        prekey_id -> Int4,
        public_key -> Bytea,
        used -> Bool,
        created_at -> Timestamp,
    }
}

diesel::table! {
    relay_messages (id) {
        id -> Int8,
        for_device_dilithium_pubkey -> Bytea,
        ciphertext -> Bytea,
        nonce -> Bytea,
        expires_at -> Timestamp,
    }
}

diesel::table! {
    sessions (session_token) {
        session_token -> Bytea,
        user_hash_id -> Bytea,
        session_expiry -> Timestamp,
        created_at -> Timestamp,
    }
}

diesel::table! {
    sync_blobs (for_device_dilithium_pubkey) {
        for_device_dilithium_pubkey -> Bytea,
        ciphertext -> Bytea,
        signature -> Bytea,
        expires_at -> Timestamp,
    }
}

diesel::table! {
    users (user_hash_id) {
        user_hash_id -> Bytea,
        password_commitment -> Bytea,
        identity_key_dilithium -> Bytea,
        identity_signature -> Bytea,
        pre_key_bundle -> Bytea,
        proof_type -> Int4,
        created_at -> Timestamp,
        recovery_dilithium_pubkey -> Nullable<Bytea>,
    }
}

diesel::joinable!(auth_challenges -> users (user_hash_id));
diesel::joinable!(friend_responses -> friend_requests (request_id));
diesel::joinable!(one_time_prekeys -> users (user_hash_id));
diesel::joinable!(sessions -> users (user_hash_id));

diesel::allow_tables_to_appear_in_same_query!(
    app_config,
    auth_challenges,
    friend_requests,
    friend_responses,
    messages,
    one_time_prekeys,
    relay_messages,
    sessions,
    sync_blobs,
    users,
);
