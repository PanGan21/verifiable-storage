use crate::constants::{
    DEFAULT_DATA_DIR, DEFAULT_HOST, DEFAULT_PORT, STORAGE_TYPE_DATABASE, STORAGE_TYPE_FILESYSTEM,
};
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
    pub fn load() -> Result<Self, std::io::Error> {
        let matches = Command::new("server")
            .arg(
                Arg::new("storage")
                    .long("storage")
                    .value_name("TYPE")
                    .help("Storage backend type: 'fs' for filesystem or 'db' for database")
                    .default_value(STORAGE_TYPE_FILESYSTEM),
            )
            .arg(
                Arg::new("data-dir")
                    .long("data-dir")
                    .value_name("DIR")
                    .help("Data directory for filesystem storage")
                    .default_value(DEFAULT_DATA_DIR),
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
            .unwrap_or(STORAGE_TYPE_FILESYSTEM);
        let storage_type = match storage_type_str {
            STORAGE_TYPE_DATABASE => StorageType::Database,
            STORAGE_TYPE_FILESYSTEM => StorageType::Filesystem,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Invalid storage type: {}. Must be '{}' or '{}'",
                        storage_type_str, STORAGE_TYPE_FILESYSTEM, STORAGE_TYPE_DATABASE
                    ),
                ));
            }
        };

        let data_dir = PathBuf::from(
            matches
                .get_one::<String>("data-dir")
                .map(|s| s.as_str())
                .unwrap_or(DEFAULT_DATA_DIR),
        );

        let database_url = if storage_type == StorageType::Database {
            Some(
                matches
                    .get_one::<String>("database-url")
                    .cloned()
                    .or_else(|| std::env::var("DATABASE_URL").ok())
                    .ok_or_else(|| {
                        error!("Database URL required when using database storage. Set --database-url or DATABASE_URL env var");
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Database URL required when using database storage. Set --database-url or DATABASE_URL env var",
                        )
                    })?,
            )
        } else {
            None
        };

        let env_host = std::env::var("SERVER_HOST").ok();
        let env_port = std::env::var("SERVER_PORT").ok();

        let host = matches
            .get_one::<String>("host")
            .map(|s| s.as_str())
            .or(env_host.as_deref())
            .unwrap_or(DEFAULT_HOST)
            .to_string();

        let port_str = matches
            .get_one::<String>("port")
            .map(|s| s.as_str())
            .or(env_port.as_deref())
            .unwrap_or(DEFAULT_PORT);

        let port = port_str.parse().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid port number: {}", port_str),
            )
        })?;

        Ok(ServerConfig {
            storage_type,
            host,
            port,
            data_dir,
            database_url,
            database_retry_config: DatabaseRetryConfig::from_env(),
        })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
