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
    /// Store a file and add it to metadata atomically
    /// For database: uses a transaction
    /// For filesystem: uses atomic file operations (temp file + fsync + rename)
    async fn store_file_with_metadata(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()>;

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

    /// Store Merkle tree structure for a batch
    async fn store_merkle_tree(
        &self,
        client_id: &str,
        batch_id: &str,
        tree: &merkle_tree::MerkleTree,
    ) -> Result<()>;

    /// Load Merkle tree structure for a batch
    async fn load_merkle_tree(
        &self,
        client_id: &str,
        batch_id: &str,
    ) -> Result<Option<merkle_tree::MerkleTree>>;

    /// Store leaf hash for a file
    async fn store_leaf_hash(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        leaf_hash: &[u8; 32],
    ) -> Result<()>;

    /// Load all leaf hashes for a batch (sorted by filename)
    async fn load_batch_leaf_hashes(
        &self,
        client_id: &str,
        batch_id: &str,
    ) -> Result<Vec<[u8; 32]>>;
}
