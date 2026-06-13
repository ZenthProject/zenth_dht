use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::{broadcast, RwLock};

static GLOBAL_CONNECTION_MANAGER: OnceLock<Arc<ConnectionManager>> = OnceLock::new();

pub fn init_global(manager: Arc<ConnectionManager>) {
    GLOBAL_CONNECTION_MANAGER.set(manager).ok();
}

pub fn get_global() -> Option<Arc<ConnectionManager>> {
    GLOBAL_CONNECTION_MANAGER.get().cloned()
}

const BROADCAST_CAPACITY: usize = 1024;

#[derive(Clone, Debug)]
pub struct BroadcastMessage {
    pub recipient_hash: Vec<u8>,
    pub payload: Vec<u8>,
}

#[derive(Clone)]
pub struct ConnectionManager {
    connections: Arc<RwLock<HashMap<Vec<u8>, broadcast::Sender<BroadcastMessage>>>>,
    global_broadcast: broadcast::Sender<BroadcastMessage>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let (global_broadcast, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            global_broadcast,
        }
    }

    pub async fn register(&self, user_hash: Vec<u8>) -> broadcast::Receiver<BroadcastMessage> {
        let mut connections = self.connections.write().await;

        if let Some(sender) = connections.get(&user_hash) {
            return sender.subscribe();
        }

        let (sender, receiver) = broadcast::channel(BROADCAST_CAPACITY);
        connections.insert(user_hash.clone(), sender);

        receiver
    }

    pub async fn unregister(&self, user_hash: &[u8]) {
        let mut connections = self.connections.write().await;
        connections.remove(user_hash);
    }

    pub async fn send_to_user(&self, recipient_hash: &[u8], payload: Vec<u8>) -> bool {
        let connections = self.connections.read().await;

        if let Some(sender) = connections.get(recipient_hash) {
            let msg = BroadcastMessage {
                recipient_hash: recipient_hash.to_vec(),
                payload,
            };

            match sender.send(msg) {
                Ok(_) => {
                    true
                }
                Err(_) => {
                    false
                }
            }
        } else {
            false
        }
    }

    pub async fn is_connected(&self, user_hash: &[u8]) -> bool {
        let connections = self.connections.read().await;
        connections.contains_key(user_hash)
    }

    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    pub fn subscribe_global(&self) -> broadcast::Receiver<BroadcastMessage> {
        self.global_broadcast.subscribe()
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
