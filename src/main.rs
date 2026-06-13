//Crate public
use axum::{
    Router,
    extract::{ws::WebSocketUpgrade, State},
    response::IntoResponse,
};
use axum_server::tls_rustls::RustlsConfig;
use dotenv::dotenv;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, private_key};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;

//project

pub mod db;
pub mod models;
pub mod schema;
pub mod crypto;
pub mod timestamp;
pub mod rate_limit;
pub mod websocket;

mod errors;
mod handlers;


//use project
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use errors::error404::fallback_handler;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
use axum::extract::DefaultBodyLimit;
use handlers::decompose::handle_post;
use websocket::{ConnectionManager, handle_websocket, init_global};

/// État partagé de l'application
#[derive(Clone)]
pub struct AppState {
    pub connection_manager: Arc<ConnectionManager>,
    pub rustfs_base_url: String,
}

//MAIN
#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Impossible d'installer le crypto provider rustls");

    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = db::init_pool(&database_url);
    db::init_global_pool(pool);

    db::establish_connection()
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    rate_limit::init_limiters();

    let host = env::var("HOST")
        .expect("Erreur : la variable d'environnement HOST n'est pas définie dans le fichier .env");
    let port = env::var("PORT")
        .expect("Erreur : la variable d'environnement PORT n'est pas définie dans le fichier .env");

    // Tâche de nettoyage : messages expirés (toutes les heures) + rate limiters (toutes les 5 min)
    tokio::spawn(async {
        let mut tick = 0u32;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            tick += 1;

            // Nettoyage des rate limiters toutes les 5 minutes
            crate::rate_limit::cleanup_all();

            // Nettoyage des entrées expirées toutes les heures (12 × 5 min)
            if tick % 12 == 0 {
                match crate::db::try_establish_connection() {
                    Ok(mut c) => {
                        use diesel::prelude::*;
                        use crate::schema::{messages, relay_messages, sync_blobs};
                        let now = chrono::Utc::now().naive_utc();

                        match diesel::delete(messages::table.filter(messages::expires_at.lt(now)))
                            .execute(&mut *c)
                        {
                            Ok(n) => if n > 0 { eprintln!("[cleanup] {} message(s) expirés supprimés", n); },
                            Err(e) => eprintln!("[cleanup] messages expirés: {}", e),
                        }

                        match diesel::delete(relay_messages::table.filter(relay_messages::expires_at.lt(now)))
                            .execute(&mut *c)
                        {
                            Ok(n) => if n > 0 { eprintln!("[cleanup] {} relay message(s) expirés supprimés", n); },
                            Err(e) => eprintln!("[cleanup] relay_messages expirés: {}", e),
                        }

                        match diesel::delete(sync_blobs::table.filter(sync_blobs::expires_at.lt(now)))
                            .execute(&mut *c)
                        {
                            Ok(n) => if n > 0 { eprintln!("[cleanup] {} sync blob(s) expirés supprimés", n); },
                            Err(e) => eprintln!("[cleanup] sync_blobs expirés: {}", e),
                        }
                    }
                    Err(e) => eprintln!("[cleanup] failed to get DB connection: {}", e),
                }
            }
        }
    });

    // Créer le gestionnaire de connexions WebSocket
    let connection_manager = Arc::new(ConnectionManager::new());

    // Initialiser le manager global pour accès depuis send_message
    init_global(connection_manager.clone());

    let rustfs_base_url = env::var("RUSTFS_BASE_URL")
        .expect("RUSTFS_BASE_URL must be set (ex: https://rustfs.example.com/zenth-updates)");

    let app_state = AppState {
        connection_manager: connection_manager.clone(),
        rustfs_base_url,
    };

    // Route "/" : POST pour HTTP, GET pour WebSocket upgrade
    let app = Router::new()
        .route("/", axum::routing::post(handle_post).get(handle_ws_upgrade))
        .fallback(fallback_handler)
        .layer(DefaultBodyLimit::max(512 * 1024)) // 512 KB
        .with_state(app_state.clone());

    let addr: SocketAddr = format!("{}:{}", host, port).parse().expect("Invalid address");

    // Check if TLS certificates exist
    let cert_path = env::var("TLS_CERT_PATH").unwrap_or_else(|_| "certs/cert.pem".to_string());
    let key_path = env::var("TLS_KEY_PATH").unwrap_or_else(|_| "certs/key.pem".to_string());

    if std::path::Path::new(&cert_path).exists() && std::path::Path::new(&key_path).exists() {
        // TLS 1.3 only configuration
        let tls_config = load_tls_config(&cert_path, &key_path)
            .expect("Failed to load TLS configuration");

        let rustls_config = RustlsConfig::from_config(tls_config);

        axum_server::bind_rustls(addr, rustls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}

/// Handler pour WebSocket upgrade (GET /)
async fn handle_ws_upgrade(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        handle_websocket(socket, state.connection_manager, state.rustfs_base_url)
    })
}

fn load_tls_config(cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error>> {
    let cert_file = File::open(cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()?;

    if certs.is_empty() {
        return Err("No certificates found in cert file".into());
    }

    let key_file = File::open(key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let key: PrivateKeyDer<'static> = private_key(&mut key_reader)?
        .ok_or("No private key found in key file")?;

    let config = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(Arc::new(config))
}
