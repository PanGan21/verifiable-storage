mod auth;
mod handlers;
mod state;

use actix_web::{web, App, HttpServer};
use clap::{Arg, Command};
use state::AppState;
use std::fs;
use std::path::PathBuf;
use storage::StorageBackend;
use tracing::{error, info};
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
        .with_writer(std::io::stderr)
        .init();

    info!(
        "Starting verifiable storage server (PID: {})",
        std::process::id()
    );

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
                .help("Server port (default: 8080, or SERVER_PORT env var)"),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .help("Server host (default: 0.0.0.0, or SERVER_HOST env var)"),
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
                error!("Database URL required when using database storage. Set --database-url or DATABASE_URL env var");
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Database URL required when using database storage. Set --database-url or DATABASE_URL env var",
                )
            })?;
        info!("Using database storage");
        
        // Get retry configuration from environment variables
        let retry_config = storage::DatabaseRetryConfig::from_env();
        info!(
            "Database retry configuration: max_attempts={}, initial_delay_seconds={}",
            retry_config.max_attempts, retry_config.initial_delay_seconds
        );
        
        info!("Connecting to database...");
        StorageBackend::Database {
            database_url: database_url.to_string(),
            retry_config: Some(retry_config),
        }
        .initialize()
        .await
        .map_err(|e| {
            error!("Failed to initialize database storage: {}", e);
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
                error!("Failed to initialize filesystem storage: {}", e);
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to initialize filesystem storage: {}", e),
                )
            })?
    };
    info!("Storage backend initialized successfully");

    // Initialize application state
    let state = web::Data::new(AppState::new(storage));

    // Get host and port from command line arguments, environment variables, or defaults
    // Priority: command-line args > environment variables > defaults
    let env_host = std::env::var("SERVER_HOST").ok();
    let env_port = std::env::var("SERVER_PORT").ok();

    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .or_else(|| env_host.as_deref())
        .unwrap_or("0.0.0.0");
    let port = matches
        .get_one::<String>("port")
        .map(|s| s.as_str())
        .or_else(|| env_port.as_deref())
        .unwrap_or("8080");
    let bind_address = format!("{}:{}", host, port);

    info!("Starting server on http://{}", bind_address);
    info!("Server state initialized, creating HttpServer...");

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(handlers::upload::upload)
            .service(handlers::download::download)
            .service(handlers::upload::health)
    })
    .bind(&bind_address)
    .map_err(|e| {
        error!("Failed to bind to {}: {}", bind_address, e);
        e
    })?;

    info!("Server bound successfully to http://{}", bind_address);

    // Run the server - this will block until the server shuts down
    // The server runs indefinitely until it receives a shutdown signal
    server.workers(1).run().await
}
