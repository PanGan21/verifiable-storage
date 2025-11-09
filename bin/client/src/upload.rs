use crate::constants::{FILENAMES_FILE, ROOT_HASH_FILE, UPLOAD_ENDPOINT};
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::{file_utils, UploadRequest};
use crypto::{hash_leaf, sign_message};
use ed25519_dalek::SigningKey;
use log::info;
use merkle_tree::MerkleTree;
use reqwest::blocking::Client;
use std::fs;
use std::path::{Path, PathBuf};

/// Get current timestamp in milliseconds since Unix epoch
/// Used to ensure each signature is unique, even for identical requests
fn get_current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Handles file uploads to the server
pub struct FileUploader {
    server: String,
    batch_id: String,
    signing_key: SigningKey,
    data_dir: PathBuf,
}

impl FileUploader {
    /// Create a new file uploader
    pub fn new(
        server: String,
        batch_id: String,
        signing_key: SigningKey,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            server,
            batch_id,
            signing_key,
            data_dir,
        }
    }
}

/// Upload files from a directory to the server
pub fn upload_files(
    dir: &Path,
    server: &str,
    batch_id: &str,
    signing_key: &SigningKey,
    data_dir: &Path,
) -> Result<String> {
    let uploader = FileUploader::new(
        server.to_string(),
        batch_id.to_string(),
        signing_key.clone(),
        data_dir.to_path_buf(),
    );
    uploader.upload_from_directory(dir)
}

impl FileUploader {
    /// Upload files from a directory
    pub fn upload_from_directory(&self, dir: &Path) -> Result<String> {
        // Read all files from directory
        let file_list = self.read_files_from_directory(dir)?;

        if file_list.is_empty() {
            anyhow::bail!("No files found in directory: {:?}", dir);
        }

        info!("Found {} files to upload", file_list.len());

        // Build Merkle tree and compute root hash
        let file_data: Vec<Vec<u8>> = file_list
            .iter()
            .map(|(_, content)| content.clone())
            .collect();
        let root_hash_hex = hex::encode(
            MerkleTree::from_data(&file_data)
                .context("Failed to build Merkle tree")?
                .root_hash(),
        );

        info!("Uploading files (computed root hash: {})", root_hash_hex);

        // Upload each file
        self.upload_files_to_server(&file_list)?;

        // Save metadata (root hash and filenames)
        self.save_upload_metadata(&root_hash_hex, &file_list)?;

        info!(
            "Upload complete. Batch ID: {}, Root hash: {}",
            self.batch_id, root_hash_hex
        );
        println!(
            "Upload complete! Batch ID: {}, Root hash: {}",
            self.batch_id, root_hash_hex
        );
        let root_hash_path = self.data_dir.join(&self.batch_id).join(ROOT_HASH_FILE);
        println!("Root hash saved to: {}", root_hash_path.display());

        Ok(root_hash_hex)
    }

    /// Read all files from a directory, sorted by filename
    fn read_files_from_directory(&self, dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
        let entries = fs::read_dir(dir).context("Failed to read directory")?;

        let mut file_list: Vec<(String, Vec<u8>)> = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());

                // Validate filename to prevent path traversal attacks
                file_utils::validate_filename(&filename)
                    .map_err(|e| anyhow::anyhow!("{}: {}", e.message(), filename))?;

                let content =
                    fs::read(&path).with_context(|| format!("Failed to read file: {:?}", path))?;
                file_list.push((filename, content));
            }
        }

        // Sort files by filename for deterministic order
        file_list.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(file_list)
    }

    /// Upload files to the server
    fn upload_files_to_server(&self, file_list: &[(String, Vec<u8>)]) -> Result<()> {
        let client = Client::new();
        let public_key = self.signing_key.verifying_key();
        let public_key_hex = hex::encode(public_key.to_bytes());

        for (filename, content) in file_list {
            let request = self.build_upload_request(filename, content, &public_key_hex)?;

            // Send request
            let url = format!("{}{}", self.server, UPLOAD_ENDPOINT);
            let response = client
                .post(&url)
                .json(&request)
                .send()
                .context("Failed to connect to server")?;

            let status = response.status();
            if !status.is_success() {
                let error_text = response
                    .text()
                    .unwrap_or_else(|_| "Unknown error".to_string());
                anyhow::bail!(
                    "Upload failed for file {}: {} - {}",
                    filename,
                    status,
                    error_text
                );
            }

            // Upload successful (HTTP status code indicates success)
            info!("Uploaded file: {}", filename);
            println!("Uploaded file: {}", filename);
        }

        Ok(())
    }

    /// Build upload request for a file
    fn build_upload_request(
        &self,
        filename: &str,
        content: &[u8],
        public_key_hex: &str,
    ) -> Result<UploadRequest> {
        // Compute leaf hash
        let leaf_hash = hash_leaf(content);
        let leaf_hash_hex = hex::encode(leaf_hash);

        // Encode file content as base64
        let file_content_b64 = STANDARD.encode(content);

        // Create message to sign
        let timestamp = self.get_current_timestamp();
        let message =
            self.build_upload_message(filename, &leaf_hash_hex, &file_content_b64, timestamp);

        // Sign message
        let signature = sign_message(&self.signing_key, &message);
        let signature_hex = hex::encode(signature.to_bytes());

        Ok(UploadRequest {
            filename: filename.to_string(),
            batch_id: self.batch_id.clone(),
            file_hash: leaf_hash_hex,
            file_content: file_content_b64,
            signature: signature_hex,
            timestamp,
            public_key: public_key_hex.to_string(),
        })
    }

    /// Build message for upload signature
    fn build_upload_message(
        &self,
        filename: &str,
        file_hash: &str,
        file_content: &str,
        timestamp: u64,
    ) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(filename.as_bytes());
        message.extend_from_slice(self.batch_id.as_bytes());
        message.extend_from_slice(file_hash.as_bytes());
        message.extend_from_slice(file_content.as_bytes());
        message.extend_from_slice(&timestamp.to_be_bytes());
        message
    }

    /// Save upload metadata (root hash and filenames)
    fn save_upload_metadata(
        &self,
        root_hash_hex: &str,
        file_list: &[(String, Vec<u8>)],
    ) -> Result<()> {
        let batch_dir = self.data_dir.join(&self.batch_id);
        fs::create_dir_all(&batch_dir).context("Failed to create batch directory")?;

        // Save root hash
        let root_hash_file = batch_dir.join(ROOT_HASH_FILE);
        fs::write(&root_hash_file, root_hash_hex)
            .with_context(|| format!("Failed to write {}", ROOT_HASH_FILE))?;

        // Save filenames
        let filenames: Vec<String> = file_list
            .iter()
            .map(|(filename, _)| filename.clone())
            .collect();
        let filenames_file = batch_dir.join(FILENAMES_FILE);
        fs::write(
            &filenames_file,
            serde_json::to_string_pretty(&filenames).context("Failed to serialize filenames")?,
        )
        .context("Failed to write filenames.json")?;

        Ok(())
    }

    /// Get current timestamp
    fn get_current_timestamp(&self) -> u64 {
        get_current_timestamp_ms()
    }
}
