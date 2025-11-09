//! Filesystem-based storage implementation

use crate::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// Filesystem-based storage implementation
pub struct FilesystemStorage {
    data_dir: PathBuf,
}

impl FilesystemStorage {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    fn get_batch_dir(&self, client_id: &str, batch_id: &str) -> PathBuf {
        self.data_dir.join(client_id).join(batch_id)
    }

    fn get_client_dir(&self, client_id: &str) -> PathBuf {
        self.data_dir.join(client_id)
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
        let batch_dir = self.get_batch_dir(client_id, batch_id);
        let file_path = batch_dir.join(filename);
        tokio::fs::create_dir_all(&batch_dir)
            .await
            .context("Failed to create batch directory")?;
        tokio::fs::write(&file_path, content)
            .await
            .context("Failed to write file")?;
        Ok(())
    }

    async fn read_file(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<Vec<u8>> {
        let file_path = self.get_batch_dir(client_id, batch_id).join(filename);
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
        let batch_dir = self.get_batch_dir(client_id, batch_id);
        let metadata_file = batch_dir.join("metadata.json");

        // Load existing metadata or create new
        let mut metadata: serde_json::Map<String, serde_json::Value> = if metadata_file.exists() {
            let content = tokio::fs::read_to_string(&metadata_file)
                .await
                .context("Failed to read metadata")?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            serde_json::Map::new()
        };

        // Get or create filenames array
        let filenames = metadata
            .entry("filenames".to_string())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));

        if let serde_json::Value::Array(ref mut arr) = filenames {
            let filename_value = serde_json::Value::String(filename.to_string());
            if !arr.contains(&filename_value) {
                arr.push(filename_value);
                // Sort by string value for deterministic order
                arr.sort_unstable_by_key(|v| v.as_str().unwrap_or("").to_string());
            }
        }

        // Write metadata back
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize metadata")?;
        tokio::fs::write(&metadata_file, metadata_json)
            .await
            .context("Failed to write metadata")?;

        Ok(())
    }

    async fn load_batch_filenames(
        &self,
        client_id: &str,
        batch_id: &str,
    ) -> Result<Vec<String>> {
        let metadata_file = self.get_batch_dir(client_id, batch_id).join("metadata.json");

        if !metadata_file.exists() {
            anyhow::bail!("Batch {} not found for client {}", batch_id, client_id);
        }

        let metadata_content = tokio::fs::read_to_string(&metadata_file)
            .await
            .context("Failed to read metadata")?;
        let metadata: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&metadata_content)
                .context("Failed to parse metadata")?;

        let filenames: Vec<String> = metadata
            .get("filenames")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Invalid metadata: missing filenames"))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        Ok(filenames)
    }

    async fn file_exists(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<bool> {
        let file_path = self.get_batch_dir(client_id, batch_id).join(filename);
        Ok(file_path.exists())
    }

    async fn read_batch_files(
        &self,
        client_id: &str,
        batch_id: &str,
        filenames: &[String],
    ) -> Result<Vec<Vec<u8>>> {
        let batch_dir = self.get_batch_dir(client_id, batch_id);
        let mut file_data = Vec::new();

        for filename in filenames {
            let file_path = batch_dir.join(filename);
            let content = tokio::fs::read(&file_path)
                .await
                .with_context(|| format!("Failed to read file {}: {:?}", filename, file_path))?;
            file_data.push(content);
        }

        Ok(file_data)
    }

    async fn store_public_key(&self, client_id: &str, public_key: &[u8]) -> Result<()> {
        let client_dir = self.get_client_dir(client_id);
        let public_key_file = client_dir.join("public_key.hex");
        tokio::fs::create_dir_all(&client_dir)
            .await
            .context("Failed to create client directory")?;
        tokio::fs::write(&public_key_file, hex::encode(public_key))
            .await
            .context("Failed to write public key")?;
        Ok(())
    }

    async fn load_public_key(&self, client_id: &str) -> Result<Option<Vec<u8>>> {
        let public_key_file = self.get_client_dir(client_id).join("public_key.hex");
        if !public_key_file.exists() {
            return Ok(None);
        }
        let public_key_hex = tokio::fs::read_to_string(&public_key_file)
            .await
            .context("Failed to read public key")?;
        let public_key_bytes = hex::decode(public_key_hex.trim())
            .context("Failed to decode public key")?;
        Ok(Some(public_key_bytes))
    }

    async fn list_client_ids(&self) -> Result<Vec<String>> {
        let mut client_ids = Vec::new();
        if !self.data_dir.exists() {
            return Ok(client_ids);
        }

        let mut entries = tokio::fs::read_dir(&self.data_dir)
            .await
            .context("Failed to read data directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let client_dir = entry.path();
            if client_dir.is_dir() {
                let public_key_file = client_dir.join("public_key.hex");
                if public_key_file.exists() {
                    if let Some(client_id) = client_dir.file_name().and_then(|n| n.to_str()) {
                        client_ids.push(client_id.to_string());
                    }
                }
            }
        }

        Ok(client_ids)
    }
}

