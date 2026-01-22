mod metadata;
use crypto::hash_leaf;
use merkle_tree::MerkleTree;

use crate::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use fs2::FileExt;
use metadata::Metadata;
use std::fs::File;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// Filesystem-based storage implementation
pub struct FilesystemStorage {
    data_dir: PathBuf,
}

impl FilesystemStorage {
    /// Create a new filesystem storage instance
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    /// Get batch directory path
    fn batch_dir(&self, client_id: &str, batch_id: &str) -> PathBuf {
        self.data_dir.join(client_id).join(batch_id)
    }

    /// Get client directory path
    fn client_dir(&self, client_id: &str) -> PathBuf {
        self.data_dir.join(client_id)
    }

    /// Get file path in batch
    fn file_path(&self, client_id: &str, batch_id: &str, filename: &str) -> PathBuf {
        self.batch_dir(client_id, batch_id).join(filename)
    }

    /// Get metadata file path
    fn metadata_path(&self, client_id: &str, batch_id: &str) -> PathBuf {
        self.batch_dir(client_id, batch_id).join("metadata.json")
    }

    /// Write file with fsync to ensure data is persisted
    async fn write_file_atomic(file_path: &PathBuf, content: &[u8]) -> Result<()> {
        // Write directly to target file and sync to ensure data is persisted
        let mut file = tokio::fs::File::create(file_path)
            .await
            .context("Failed to create file")?;

        file.write_all(content)
            .await
            .context("Failed to write content to file")?;

        // Sync file data to disk to ensure it's persisted
        file.sync_all()
            .await
            .context("Failed to sync file to disk")?;

        Ok(())
    }

    /// Get public key file path
    fn public_key_path(&self, client_id: &str) -> PathBuf {
        self.client_dir(client_id).join("public_key.hex")
    }

    /// Get Merkle tree file path
    fn merkle_tree_path(&self, client_id: &str, batch_id: &str) -> PathBuf {
        self.batch_dir(client_id, batch_id).join("merkle_tree.json")
    }

    /// Get lock file path for batch-level locking
    fn lock_file_path(&self, client_id: &str, batch_id: &str) -> PathBuf {
        self.batch_dir(client_id, batch_id).join(".lock")
    }
}

#[async_trait]
impl Storage for FilesystemStorage {
    async fn read_file(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<Vec<u8>> {
        let file_path = self.file_path(client_id, batch_id, filename);
        tokio::fs::read(&file_path)
            .await
            .with_context(|| format!("Failed to read file: {:?}", file_path))
    }

    async fn load_batch_filenames(&self, client_id: &str, batch_id: &str) -> Result<Vec<String>> {
        let metadata_file = self.metadata_path(client_id, batch_id);

        if !metadata_file.exists() {
            anyhow::bail!("Batch {} not found for client {}", batch_id, client_id);
        }

        Metadata::load_filenames(&metadata_file).await
    }

    async fn file_exists(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<bool> {
        let file_path = self.file_path(client_id, batch_id, filename);
        Ok(file_path.exists())
    }

    async fn store_public_key(&self, client_id: &str, public_key: &[u8]) -> Result<()> {
        let client_dir = self.client_dir(client_id);
        let public_key_file = self.public_key_path(client_id);

        tokio::fs::create_dir_all(&client_dir)
            .await
            .context("Failed to create client directory")?;
        tokio::fs::write(&public_key_file, hex::encode(public_key))
            .await
            .context("Failed to write public key")?;
        Ok(())
    }

    async fn load_public_key(&self, client_id: &str) -> Result<Option<Vec<u8>>> {
        let public_key_file = self.public_key_path(client_id);

        if !public_key_file.exists() {
            return Ok(None);
        }

        let public_key_hex = tokio::fs::read_to_string(&public_key_file)
            .await
            .context("Failed to read public key")?;
        let public_key_bytes =
            hex::decode(public_key_hex.trim()).context("Failed to decode public key")?;
        Ok(Some(public_key_bytes))
    }

    async fn load_merkle_tree(
        &self,
        client_id: &str,
        batch_id: &str,
    ) -> Result<Option<merkle_tree::MerkleTree>> {
        let tree_file = self.merkle_tree_path(client_id, batch_id);

        if !tree_file.exists() {
            return Ok(None);
        }

        let tree_json = tokio::fs::read_to_string(&tree_file)
            .await
            .context("Failed to read Merkle tree file")?;
        let tree: merkle_tree::MerkleTree =
            serde_json::from_str(&tree_json).context("Failed to deserialize Merkle tree")?;

        Ok(Some(tree))
    }

    async fn store_file_and_update_tree(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()> {
        let batch_dir = self.batch_dir(client_id, batch_id);
        let lock_file = self.lock_file_path(client_id, batch_id);

        // Create batch directory if it doesn't exist
        tokio::fs::create_dir_all(&batch_dir)
            .await
            .context("Failed to create batch directory")?;

        // Acquire exclusive lock on the batch
        // This prevents concurrent modifications from other processes/servers
        let lock_file_handle = tokio::task::spawn_blocking({
            let lock_file = lock_file.clone();
            move || {
                // Create lock file if it doesn't exist
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(&lock_file)
                    .context("Failed to create lock file")?;

                // Acquire exclusive lock (blocks until available)
                file.lock_exclusive()
                    .context("Failed to acquire exclusive lock")?;

                Ok::<_, anyhow::Error>(file)
            }
        })
        .await
        .context("Failed to spawn blocking task for file lock")?
        .context("Failed to acquire file lock")?;

        // Ensure lock is released when all done
        let _guard = LockGuard(lock_file_handle);

        // Store file
        let file_path = self.file_path(client_id, batch_id, filename);
        Self::write_file_atomic(&file_path, content)
            .await
            .context("Failed to write file atomically")?;

        // Update metadata
        let metadata_file = self.metadata_path(client_id, batch_id);
        let mut metadata = if metadata_file.exists() {
            Metadata::load(&metadata_file).await?
        } else {
            serde_json::Map::new()
        };
        Metadata::insert_filename(&mut metadata, filename);
        Metadata::save_atomic(&metadata_file, &metadata)
            .await
            .context("Failed to write metadata atomically")?;

        // Load all filenames (sorted) including the newly uploaded file
        let filenames = Metadata::load_filenames(&metadata_file).await?;

        // Compute leaf hashes from all files
        let mut leaf_hashes = Vec::new();
        for filename in &filenames {
            let file_path = self.file_path(client_id, batch_id, filename);
            let file_content = tokio::fs::read(&file_path)
                .await
                .with_context(|| format!("Failed to read file: {:?}", file_path))?;
            let leaf_hash = hash_leaf(&file_content);
            leaf_hashes.push(leaf_hash);
        }

        // Build Merkle tree from all leaf hashes
        let tree = MerkleTree::from_leaf_hashes(&leaf_hashes)
            .context("Failed to build Merkle tree from leaf hashes")?;

        // Store the rebuilt tree
        let tree_file = self.merkle_tree_path(client_id, batch_id);
        let tree_json =
            serde_json::to_string_pretty(&tree).context("Failed to serialize Merkle tree")?;
        Self::write_file_atomic(&tree_file, tree_json.as_bytes())
            .await
            .context("Failed to write Merkle tree file")?;

        Ok(())
    }
}

/// Guard to ensure file lock is released
struct LockGuard(File);

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
