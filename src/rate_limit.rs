use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// relay_push : 60 requêtes par minute par clé expéditeur
const RELAY_PUSH_MAX: u32  = 60;
const RELAY_PUSH_WINDOW: u64 = 60;

// sync_blob push : 20 requêtes par minute
const BLOB_PUSH_MAX: u32   = 20;
const BLOB_PUSH_WINDOW: u64 = 60;

pub struct KeyRateLimiter {
    // Clé : 32 premiers octets de la pubkey Dilithium (évite de stocker 1312 bytes)
    map: Mutex<HashMap<[u8; 32], (Instant, u32)>>,
    max_requests: u32,
    window_secs: u64,
}

impl KeyRateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            max_requests,
            window_secs,
        }
    }

    /// Retourne `true` si la requête est autorisée, `false` si la limite est atteinte.
    pub fn is_allowed(&self, key: &[u8]) -> bool {
        if key.is_empty() {
            return false;
        }
        let mut k = [0u8; 32];
        let len = key.len().min(32);
        k[..len].copy_from_slice(&key[..len]);

        let now = Instant::now();
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let entry = map.entry(k).or_insert((now, 0));

        if entry.0.elapsed().as_secs() >= self.window_secs {
            *entry = (now, 1);
            true
        } else if entry.1 < self.max_requests {
            entry.1 += 1;
            true
        } else {
            false
        }
    }

    /// Supprime les entrées inactives depuis 2× la fenêtre (appelé périodiquement).
    pub fn cleanup(&self) {
        let threshold = self.window_secs * 2;
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, (instant, _)| instant.elapsed().as_secs() < threshold);
    }
}

static RELAY_PUSH_LIMITER: OnceLock<KeyRateLimiter> = OnceLock::new();
static BLOB_PUSH_LIMITER:  OnceLock<KeyRateLimiter> = OnceLock::new();

pub fn init_limiters() {
    RELAY_PUSH_LIMITER
        .set(KeyRateLimiter::new(RELAY_PUSH_MAX, RELAY_PUSH_WINDOW))
        .ok();
    BLOB_PUSH_LIMITER
        .set(KeyRateLimiter::new(BLOB_PUSH_MAX, BLOB_PUSH_WINDOW))
        .ok();
}

/// Vérifie le rate limit pour relay_push. Clé = sender_dilithium_pubkey.
pub fn relay_push_allowed(sender_key: &[u8]) -> bool {
    RELAY_PUSH_LIMITER
        .get()
        .map(|l| l.is_allowed(sender_key))
        .unwrap_or(true)
}

/// Vérifie le rate limit pour sync_blob push. Clé = sender_dilithium_pubkey.
pub fn blob_push_allowed(sender_key: &[u8]) -> bool {
    BLOB_PUSH_LIMITER
        .get()
        .map(|l| l.is_allowed(sender_key))
        .unwrap_or(true)
}

/// Nettoyage périodique des deux limiters.
pub fn cleanup_all() {
    if let Some(l) = RELAY_PUSH_LIMITER.get() { l.cleanup(); }
    if let Some(l) = BLOB_PUSH_LIMITER.get()  { l.cleanup(); }
}
