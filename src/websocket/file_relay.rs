use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

type TransferId = [u8; 16];

const TRANSFER_TTL: Duration = Duration::from_secs(10 * 60); // 10 minutes

struct Entry {
    sender_hash: Vec<u8>,
    inserted_at: Instant,
}

static REGISTRY: OnceLock<Arc<RwLock<HashMap<TransferId, Entry>>>> = OnceLock::new();

pub fn init() {
    REGISTRY.set(Arc::new(RwLock::new(HashMap::new()))).ok();
}

fn registry() -> Arc<RwLock<HashMap<TransferId, Entry>>> {
    REGISTRY.get().expect("file_relay not initialised").clone()
}

/// Enregistre un transfert : l'expéditeur annonce qu'il a les chunks prêts.
pub async fn register(transfer_id: TransferId, sender_hash: Vec<u8>) {
    let arc = registry();
    let mut map = arc.write().await;
    map.insert(transfer_id, Entry { sender_hash, inserted_at: Instant::now() });
}

/// Retire un transfert du registre (fin normale).
pub async fn unregister(transfer_id: &TransferId) {
    let arc = registry();
    let mut map = arc.write().await;
    map.remove(transfer_id);
}

/// Retire tous les transferts associés à un expéditeur (déconnexion).
pub async fn unregister_by_sender(sender_hash: &[u8]) {
    let arc = registry();
    let mut map = arc.write().await;
    map.retain(|_, entry| entry.sender_hash != sender_hash);
}

/// Retourne le hash de l'expéditeur pour ce transfer_id, ou None si inconnu ou expiré.
pub async fn get_sender(transfer_id: &TransferId) -> Option<Vec<u8>> {
    let arc = registry();
    let map = arc.read().await;
    map.get(transfer_id)
        .filter(|e| e.inserted_at.elapsed() < TRANSFER_TTL)
        .map(|e| e.sender_hash.clone())
}

/// Supprime les entrées expirées (appelé par la tâche de cleanup périodique).
pub async fn cleanup_expired() {
    let arc = registry();
    let mut map = arc.write().await;
    map.retain(|_, entry| entry.inserted_at.elapsed() < TRANSFER_TTL);
}

pub fn parse_transfer_id(raw: &[u8]) -> Option<TransferId> {
    raw.try_into().ok()
}
