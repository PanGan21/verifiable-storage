//! Storage backend initialization

use anyhow::Result;
use std::sync::Arc;
use storage::{database::DatabaseStorage, filesystem::FilesystemStorage, Storage};

/// Storage backend type
pub enum StorageBackend {
    Filesystem(String), // data directory path
    Database(String),   // database URL
}

impl StorageBackend {
    /// Initialize storage backend based on type
    pub async fn initialize(self) -> Result<Arc<dyn Storage>> {
        match self {
            StorageBackend::Filesystem(data_dir) => {
                let storage = FilesystemStorage::new(data_dir);
                Ok(Arc::new(storage))
            }
            StorageBackend::Database(database_url) => {
                let storage = DatabaseStorage::new(&database_url).await?;
                Ok(Arc::new(storage))
            }
        }
    }
}

