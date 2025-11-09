//! Verifiable storage server

mod auth;
mod handlers;
mod state;
mod storage_backend;

use actix_web::{web, App, HttpServer};
use clap::{Arg, Command};
use state::AppState;
use std::fs;
use std::path::PathBuf;
use storage_backend::StorageBackend;
use tracing::info;
use tracing_subscriber;

const SERVER_DATA_DIR: &str = "server_data";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing with env filter
    // Filter out actix-server worker shutdown messages
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info")
                    .add_directive("actix_server::worker=warn".parse().unwrap())
                    .add_directive("actix_server::accept=warn".parse().unwrap())
            }),
        )
        .init();

    // Parse command line arguments
    let matches = Command::new("server")
        .arg(
            Arg::new("storage")
                .long("storage")
                .value_name("TYPE")
                .help("Storage backend type: 'fs' for filesystem or 'db' for database")
                .default_value("fs"),
        )
        .arg(
            Arg::new("data-dir")
                .long("data-dir")
                .value_name("DIR")
                .help("Data directory for filesystem storage")
                .default_value(SERVER_DATA_DIR),
        )
        .arg(
            Arg::new("database-url")
                .long("database-url")
                .value_name("URL")
                .help("Database URL for database storage (can also use DATABASE_URL env var)"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .value_name("PORT")
                .help("Server port (default: 8080)")
                .default_value("8080"),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .help("Server host (default: 0.0.0.0)")
                .default_value("0.0.0.0"),
        )
        .get_matches();

    // Initialize storage backend
    let storage = if matches.get_one::<String>("storage").map(|s| s.as_str()) == Some("db") {
        // Get database URL as owned String
        let database_url = matches
            .get_one::<String>("database-url")
            .cloned()
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Database URL required when using database storage. Set --database-url or DATABASE_URL env var",
                )
            })?;
        info!("Using database storage: {}", database_url);
        StorageBackend::Database(database_url.to_string())
            .initialize()
            .await
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to initialize database storage: {}", e),
                )
            })?
    } else {
        let data_dir = matches
            .get_one::<String>("data-dir")
            .map(|s| s.as_str())
            .unwrap_or(SERVER_DATA_DIR);
        let data_dir_path = PathBuf::from(data_dir);
        if !data_dir_path.exists() {
            fs::create_dir_all(&data_dir_path)?;
        }
        info!("Using filesystem storage: {:?}", data_dir_path);
        StorageBackend::Filesystem(data_dir.to_string())
            .initialize()
            .await
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to initialize filesystem storage: {}", e),
                )
            })?
    };

    // Initialize application state
    let state = web::Data::new(AppState::new(storage));

    // Get host and port from command line arguments
    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .unwrap_or("0.0.0.0");
    let port = matches
        .get_one::<String>("port")
        .map(|s| s.as_str())
        .unwrap_or("8080");
    let bind_address = format!("{}:{}", host, port);

    info!("Starting server on http://{}", bind_address);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(handlers::upload)
            .service(handlers::download)
            .service(handlers::health)
    })
    .bind(&bind_address)?
    .workers(1) // Use single worker to reduce shutdown logs
    .run()
    .await
}
