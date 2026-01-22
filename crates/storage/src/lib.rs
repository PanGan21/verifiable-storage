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
    /// Read a file from a batch
    async fn read_file(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<Vec<u8>>;

    /// Load all filenames from batch metadata
    async fn load_batch_filenames(&self, client_id: &str, batch_id: &str) -> Result<Vec<String>>;

    /// Check if a file exists in a batch
    async fn file_exists(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<bool>;

    /// Store or update a client's public key
    async fn store_public_key(&self, client_id: &str, public_key: &[u8]) -> Result<()>;

    /// Load a client's public key
    async fn load_public_key(&self, client_id: &str) -> Result<Option<Vec<u8>>>;

    /// Load Merkle tree structure for a batch
    async fn load_merkle_tree(
        &self,
        client_id: &str,
        batch_id: &str,
    ) -> Result<Option<merkle_tree::MerkleTree>>;

    /// Atomically store file and update Merkle tree
    /// This method ensures that concurrent uploads to the same batch_id are handled correctly
    /// by using transactions and locking to prevent race conditions.
    /// For database: uses a transaction with SELECT FOR UPDATE
    /// For filesystem: uses file locking
    async fn store_file_and_update_tree(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()>;
}
