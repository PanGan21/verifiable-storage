use crate::{
    database::{DatabaseRetryConfig, DatabaseStorage},
    filesystem::FilesystemStorage,
    Storage,
};
use anyhow::Result;
use std::sync::Arc;

/// Storage backend type
pub enum StorageBackend {
    /// Filesystem storage with data directory path
    Filesystem(String),
    /// Database storage with database URL and optional retry configuration
    Database {
        database_url: String,
        retry_config: Option<DatabaseRetryConfig>,
    },
}

impl StorageBackend {
    /// Initialize storage backend based on type
    pub async fn initialize(self) -> Result<Arc<dyn Storage>> {
        match self {
            StorageBackend::Filesystem(data_dir) => {
                let storage = FilesystemStorage::new(data_dir);
                Ok(Arc::new(storage))
            }
            StorageBackend::Database {
                database_url,
                retry_config,
            } => {
                let storage = match retry_config {
                    Some(config) => {
                        DatabaseStorage::new_with_retry_config(&database_url, config).await?
                    }
                    None => DatabaseStorage::new(&database_url).await?,
                };
                Ok(Arc::new(storage))
            }
        }
    }
}
