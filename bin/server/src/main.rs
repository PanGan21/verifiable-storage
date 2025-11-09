mod auth;
mod config;
mod constants;
mod handlers;
mod logger;
mod proof;
mod state;

use actix_web::{web, App, HttpServer};
use config::ServerConfig;
use logger::init as init_logger;
use state::AppState;
use storage::StorageBackend;
use tracing::{error, info};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_logger();

    info!(
        "Starting verifiable storage server (PID: {})",
        std::process::id()
    );

    let config = ServerConfig::load()?;

    let storage = match config.storage_type {
        config::StorageType::Database => {
            let database_url = config.database_url.as_ref().unwrap();
            info!("Using database storage");
            info!(
                "Database retry configuration: max_attempts={}, initial_delay_seconds={}",
                config.database_retry_config.max_attempts,
                config.database_retry_config.initial_delay_seconds
            );
            StorageBackend::Database {
                database_url: database_url.clone(),
                retry_config: Some(config.database_retry_config.clone()),
            }
            .initialize()
            .await
            .map_err(|e| {
                error!("Failed to initialize database storage: {}", e);
                std::io::Error::other(format!("Failed to initialize database storage: {}", e))
            })?
        }
        config::StorageType::Filesystem => {
            if !config.data_dir.exists() {
                std::fs::create_dir_all(&config.data_dir)?;
            }
            info!("Using filesystem storage: {:?}", config.data_dir);
            StorageBackend::Filesystem(
                config
                    .data_dir
                    .to_str()
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Invalid data directory path",
                        )
                    })?
                    .to_string(),
            )
            .initialize()
            .await
            .map_err(|e| {
                error!("Failed to initialize filesystem storage: {}", e);
                std::io::Error::other(format!("Failed to initialize filesystem storage: {}", e))
            })?
        }
    };
    info!("Storage backend initialized successfully");

    let state = web::Data::new(AppState::new(storage));
    let bind_address = config.bind_address();

    info!("Starting server on http://{}", bind_address);

    let bind_addr = bind_address.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(handlers::upload::upload)
            .service(handlers::download::download)
            .service(handlers::health::health)
    })
    .bind(&bind_addr)
    .map_err(|e| {
        error!("Failed to bind to {}: {}", bind_addr, e);
        e
    })?;

    info!("Server bound successfully to http://{}", bind_address);
    server.workers(1).run().await
}
