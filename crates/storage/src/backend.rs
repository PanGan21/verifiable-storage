use crate::{database::DatabaseStorage, filesystem::FilesystemStorage, Storage};
use anyhow::Result;
use std::sync::Arc;

/// Storage backend type
pub enum StorageBackend {
    /// Filesystem storage with data directory path
    Filesystem(String),
    /// Database storage with database URL
    Database(String),
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
