use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::Path;
use tokio::io::AsyncWriteExt;

/// Filesystem metadata manager
pub struct Metadata;

impl Metadata {
    /// Add filename to metadata file
    pub async fn add_filename(metadata_file: &Path, filename: &str) -> Result<()> {
        let mut metadata = Self::load_or_create(metadata_file).await?;
        Self::insert_filename(&mut metadata, filename);
        Self::save(metadata_file, &metadata).await?;
        Ok(())
    }

    /// Load filenames from metadata file
    pub async fn load_filenames(metadata_file: &Path) -> Result<Vec<String>> {
        let metadata = Self::load(metadata_file).await?;
        Self::extract_filenames(&metadata)
    }

    /// Load metadata or create empty metadata
    async fn load_or_create(metadata_file: &Path) -> Result<Map<String, Value>> {
        if metadata_file.exists() {
            Self::load(metadata_file).await
        } else {
            Ok(Map::new())
        }
    }

    /// Load metadata from file (public for use in atomic operations)
    pub async fn load(metadata_file: &Path) -> Result<Map<String, Value>> {
        let content = tokio::fs::read_to_string(metadata_file)
            .await
            .context("Failed to read metadata")?;
        serde_json::from_str(&content).context("Failed to parse metadata")
    }

    /// Insert filename into metadata (public for use in atomic operations)
    pub fn insert_filename(metadata: &mut Map<String, Value>, filename: &str) {
        let filenames = metadata
            .entry("filenames".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));

        if let Value::Array(ref mut arr) = filenames {
            let filename_value = Value::String(filename.to_string());
            if !arr.contains(&filename_value) {
                arr.push(filename_value);
                // Sort by string value for deterministic order
                arr.sort_unstable_by_key(|v| v.as_str().unwrap_or("").to_string());
            }
        }
    }

    /// Extract filenames from metadata
    fn extract_filenames(metadata: &Map<String, Value>) -> Result<Vec<String>> {
        metadata
            .get("filenames")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Invalid metadata: missing filenames"))
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
    }

    /// Save metadata to file (non-atomic, for backwards compatibility)
    async fn save(metadata_file: &Path, metadata: &Map<String, Value>) -> Result<()> {
        let metadata_json =
            serde_json::to_string_pretty(metadata).context("Failed to serialize metadata")?;
        tokio::fs::write(metadata_file, metadata_json)
            .await
            .context("Failed to write metadata")?;
        Ok(())
    }

    /// Save metadata to file with fsync to ensure data is persisted
    pub async fn save_atomic(metadata_file: &Path, metadata: &Map<String, Value>) -> Result<()> {
        // Serialize metadata to JSON
        let metadata_json =
            serde_json::to_string_pretty(metadata).context("Failed to serialize metadata")?;

        // Write directly to metadata file
        let mut file = tokio::fs::File::create(metadata_file)
            .await
            .context("Failed to create metadata file")?;

        file.write_all(metadata_json.as_bytes())
            .await
            .context("Failed to write metadata to file")?;

        // Sync file data to disk to ensure it's persisted
        file.sync_all()
            .await
            .context("Failed to sync metadata file to disk")?;

        Ok(())
    }
}
