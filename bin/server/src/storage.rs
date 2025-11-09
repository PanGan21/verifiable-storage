//! File storage and metadata management

use anyhow::{Context, Result};
use serde_json;
use std::fs;
use std::path::PathBuf;

const SERVER_DATA_DIR: &str = "server_data";

/// Manages file storage and metadata for batches
pub struct BatchStorage;

impl BatchStorage {
    /// Get the batch directory path for a client and batch
    pub fn get_batch_dir(client_id: &str, batch_id: &str) -> PathBuf {
        PathBuf::from(SERVER_DATA_DIR)
            .join(client_id)
            .join(batch_id)
    }

    /// Store a file in the batch directory
    pub fn store_file(
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()> {
        let batch_dir = Self::get_batch_dir(client_id, batch_id);
        let file_path = batch_dir.join(filename);
        fs::create_dir_all(&batch_dir).context("Failed to create batch directory")?;
        fs::write(&file_path, content).context("Failed to write file")?;
        Ok(())
    }

    /// Read a file from the batch directory
    pub fn read_file(client_id: &str, batch_id: &str, filename: &str) -> Result<Vec<u8>> {
        let file_path = Self::get_batch_dir(client_id, batch_id).join(filename);
        fs::read(&file_path)
            .with_context(|| format!("Failed to read file: {:?}", file_path))
    }

    /// Update metadata to add a filename to a batch
    pub fn add_filename_to_metadata(
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<()> {
        let batch_dir = Self::get_batch_dir(client_id, batch_id);
        let metadata_file = batch_dir.join("metadata.json");

        // Load existing metadata or create new
        let mut metadata: serde_json::Map<String, serde_json::Value> = if metadata_file.exists() {
            let content = fs::read_to_string(&metadata_file)
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
        fs::write(&metadata_file, metadata_json)
            .context("Failed to write metadata")?;

        Ok(())
    }

    /// Load filenames from batch metadata
    pub fn load_batch_filenames(client_id: &str, batch_id: &str) -> Result<Vec<String>> {
        let metadata_file = Self::get_batch_dir(client_id, batch_id).join("metadata.json");

        if !metadata_file.exists() {
            anyhow::bail!("Batch {} not found for client {}", batch_id, client_id);
        }

        let metadata_content = fs::read_to_string(&metadata_file)
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

    /// Check if file exists in batch
    pub fn file_exists(client_id: &str, batch_id: &str, filename: &str) -> bool {
        Self::get_batch_dir(client_id, batch_id)
            .join(filename)
            .exists()
    }

    /// Read all files from a batch in sorted order
    pub fn read_batch_files(
        client_id: &str,
        batch_id: &str,
        filenames: &[String],
    ) -> Result<Vec<Vec<u8>>> {
        let batch_dir = Self::get_batch_dir(client_id, batch_id);
        let mut file_data = Vec::new();

        for filename in filenames {
            let file_path = batch_dir.join(filename);
            let content = fs::read(&file_path)
                .with_context(|| format!("Failed to read file {}: {:?}", filename, file_path))?;
            file_data.push(content);
        }

        Ok(file_data)
    }
}
