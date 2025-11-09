//! Verifiable storage server

mod auth;
mod handlers;
mod state;
mod storage;

use actix_web::{web, App, HttpServer};
use state::AppState;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};
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

    let args: Vec<String> = std::env::args().collect();
    let data_dir = args.get(1).map(|s| s.as_str()).unwrap_or(SERVER_DATA_DIR);
    let data_dir_path = PathBuf::from(data_dir);
    if !data_dir_path.exists() {
        fs::create_dir_all(&data_dir_path)?;
    }

    let state = web::Data::new(AppState::new());
    if let Err(e) = state.load_from_disk() {
        warn!("Failed to load state from disk: {}", e);
    }

    info!("Starting server on http://127.0.0.1:8080");
    info!("Data directory: {:?}", data_dir);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(handlers::upload)
            .service(handlers::download)
            .service(handlers::health)
    })
    .bind("127.0.0.1:8080")?
    .workers(1) // Use single worker to reduce shutdown logs
    .run()
    .await
}
