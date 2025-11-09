pub mod backend;
pub mod database;
pub mod filesystem;

use anyhow::Result;
use async_trait::async_trait;

pub use backend::StorageBackend;
pub use database::DatabaseRetryConfig;

/// Storage backend trait for file and metadata operations
#[async_trait]
pub trait Storage: Send + Sync {
    /// Store a file in a batch
    async fn store_file(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()>;

    /// Read a file from a batch
    async fn read_file(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<Vec<u8>>;

    /// Add a filename to batch metadata
    async fn add_filename_to_metadata(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<()>;

    /// Load all filenames from batch metadata
    async fn load_batch_filenames(&self, client_id: &str, batch_id: &str) -> Result<Vec<String>>;

    /// Check if a file exists in a batch
    async fn file_exists(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<bool>;

    /// Read all files from a batch in sorted order
    async fn read_batch_files(
        &self,
        client_id: &str,
        batch_id: &str,
        filenames: &[String],
    ) -> Result<Vec<Vec<u8>>>;

    /// Store or update a client's public key
    async fn store_public_key(&self, client_id: &str, public_key: &[u8]) -> Result<()>;

    /// Load a client's public key
    async fn load_public_key(&self, client_id: &str) -> Result<Option<Vec<u8>>>;

    /// List all client IDs that have public keys stored
    async fn list_client_ids(&self) -> Result<Vec<String>>;
}
