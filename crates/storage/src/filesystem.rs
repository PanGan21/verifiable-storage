mod metadata;
use merkle_tree::MerkleTree;
use std::collections::HashMap;

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

    /// Get leaf hashes file path
    fn leaf_hashes_path(&self, client_id: &str, batch_id: &str) -> PathBuf {
        self.batch_dir(client_id, batch_id).join("leaf_hashes.json")
    }

    /// Get lock file path for a batch (used for file locking)
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
        leaf_hash: &[u8; 32],
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

        // Store leaf hash
        let leaf_hashes_file = self.leaf_hashes_path(client_id, batch_id);
        let mut leaf_hashes: HashMap<String, String> = if leaf_hashes_file.exists() {
            let content = tokio::fs::read_to_string(&leaf_hashes_file)
                .await
                .context("Failed to read leaf hashes file")?;
            serde_json::from_str(&content).context("Failed to parse leaf hashes file")?
        } else {
            HashMap::new()
        };

        leaf_hashes.insert(filename.to_string(), hex::encode(leaf_hash));
        let leaf_hashes_json = serde_json::to_string_pretty(&leaf_hashes)
            .context("Failed to serialize leaf hashes")?;
        Self::write_file_atomic(&leaf_hashes_file, leaf_hashes_json.as_bytes())
            .await
            .context("Failed to write leaf hashes file")?;

        // Load all leaf hashes (sorted by filename)
        let mut sorted_entries: Vec<_> = leaf_hashes.into_iter().collect();
        sorted_entries.sort_by_key(|(filename, _)| filename.clone());

        let mut all_leaf_hashes = Vec::new();
        for (_, hash_hex) in sorted_entries {
            let hash_bytes = hex::decode(hash_hex.trim()).context("Failed to decode leaf hash")?;
            if hash_bytes.len() != 32 {
                anyhow::bail!(
                    "Invalid leaf hash length: expected 32 bytes, got {}",
                    hash_bytes.len()
                );
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);
            all_leaf_hashes.push(hash);
        }

        // Build Merkle tree from all leaf hashes
        let tree = MerkleTree::from_leaf_hashes(&all_leaf_hashes)
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
