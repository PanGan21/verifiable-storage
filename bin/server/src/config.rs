use clap::{Arg, Command};
use std::path::PathBuf;
use storage::DatabaseRetryConfig;
use tracing::error;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Storage backend type
    pub storage_type: StorageType,
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Data directory for filesystem storage
    pub data_dir: PathBuf,
    /// Database URL for database storage
    pub database_url: Option<String>,
    /// Database retry configuration
    pub database_retry_config: DatabaseRetryConfig,
}

/// Storage backend type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageType {
    Filesystem,
    Database,
}

impl ServerConfig {
    /// Load configuration from command line arguments and environment variables
    /// Priority: command-line args > environment variables > defaults
    pub fn load() -> Result<Self, std::io::Error> {
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
                    .default_value("server_data"),
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

        // Determine storage type
        let storage_type_str = matches
            .get_one::<String>("storage")
            .map(|s| s.as_str())
            .unwrap_or("fs");
        let storage_type = match storage_type_str {
            "db" => StorageType::Database,
            "fs" => StorageType::Filesystem,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Invalid storage type: {}. Must be 'fs' or 'db'",
                        storage_type_str
                    ),
                ));
            }
        };

        // Get data directory (for filesystem storage)
        let data_dir = matches
            .get_one::<String>("data-dir")
            .map(|s| s.as_str())
            .unwrap_or("server_data");
        let data_dir_path = PathBuf::from(data_dir);

        // Get database URL (for database storage)
        let database_url = if storage_type == StorageType::Database {
            let url = matches
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
            Some(url)
        } else {
            None
        };

        // Get host and port from command line arguments, environment variables, or defaults
        let env_host = std::env::var("SERVER_HOST").ok();
        let env_port = std::env::var("SERVER_PORT").ok();

        let host = matches
            .get_one::<String>("host")
            .map(|s| s.as_str())
            .or_else(|| env_host.as_deref())
            .unwrap_or("0.0.0.0")
            .to_string();

        let port_str = matches
            .get_one::<String>("port")
            .map(|s| s.as_str())
            .or_else(|| env_port.as_deref())
            .unwrap_or("8080");

        let port = port_str.parse().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid port number: {}", port_str),
            )
        })?;

        // Get database retry configuration from environment variables
        let database_retry_config = DatabaseRetryConfig::from_env();

        Ok(ServerConfig {
            storage_type,
            host,
            port,
            data_dir: data_dir_path,
            database_url,
            database_retry_config,
        })
    }

    /// Get bind address as a string
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
