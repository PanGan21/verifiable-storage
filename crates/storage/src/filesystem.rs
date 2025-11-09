mod metadata;

use crate::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use metadata::Metadata;
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
}

#[async_trait]
impl Storage for FilesystemStorage {
    async fn store_file(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()> {
        let batch_dir = self.batch_dir(client_id, batch_id);
        let file_path = self.file_path(client_id, batch_id, filename);

        tokio::fs::create_dir_all(&batch_dir)
            .await
            .context("Failed to create batch directory")?;
        Self::write_file_atomic(&file_path, content).await?;
        Ok(())
    }

    async fn store_file_with_metadata(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()> {
        let batch_dir = self.batch_dir(client_id, batch_id);
        let file_path = self.file_path(client_id, batch_id, filename);
        let metadata_file = self.metadata_path(client_id, batch_id);

        // Create batch directory if it doesn't exist
        tokio::fs::create_dir_all(&batch_dir)
            .await
            .context("Failed to create batch directory")?;

        // Load or create metadata
        let mut metadata = if metadata_file.exists() {
            Metadata::load(&metadata_file).await?
        } else {
            serde_json::Map::new()
        };

        Metadata::insert_filename(&mut metadata, filename);

        // Write file atomically
        Self::write_file_atomic(&file_path, content)
            .await
            .context("Failed to write file atomically")?;

        // Write metadata atomically
        Metadata::save_atomic(&metadata_file, &metadata)
            .await
            .context("Failed to write metadata atomically")?;

        Ok(())
    }

    async fn read_file(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<Vec<u8>> {
        let file_path = self.file_path(client_id, batch_id, filename);
        tokio::fs::read(&file_path)
            .await
            .with_context(|| format!("Failed to read file: {:?}", file_path))
    }

    async fn add_filename_to_metadata(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<()> {
        let metadata_file = self.metadata_path(client_id, batch_id);
        Metadata::add_filename(&metadata_file, filename).await
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

    async fn read_batch_files(
        &self,
        client_id: &str,
        batch_id: &str,
        filenames: &[String],
    ) -> Result<Vec<Vec<u8>>> {
        let mut file_data = Vec::new();

        for filename in filenames {
            let file_path = self.file_path(client_id, batch_id, filename);
            let content = tokio::fs::read(&file_path)
                .await
                .with_context(|| format!("Failed to read file {}: {:?}", filename, file_path))?;
            file_data.push(content);
        }

        Ok(file_data)
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

    async fn list_client_ids(&self) -> Result<Vec<String>> {
        if !self.data_dir.exists() {
            return Ok(Vec::new());
        }

        let mut client_ids = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.data_dir)
            .await
            .context("Failed to read data directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let client_dir = entry.path();
            if client_dir.is_dir() {
                if let Some(client_id) = client_dir.file_name().and_then(|n| n.to_str()) {
                    let public_key_file = self.public_key_path(client_id);
                    if public_key_file.exists() {
                        client_ids.push(client_id.to_string());
                    }
                }
            }
        }

        Ok(client_ids)
    }
}
