use crate::constants::{DOWNLOADED_DIR, DOWNLOAD_ENDPOINT, ROOT_HASH_FILE};
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::{file_utils, DownloadResponse, ProofNodeJson};
use crypto::{hash_leaf, sign_message};
use ed25519_dalek::SigningKey;
use merkle_tree::MerkleProof;
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

/// Helper to decode hex string to fixed-size array
fn hex_decode_array<const N: usize>(s: &str) -> Result<[u8; N]> {
    let bytes = hex::decode(s.trim())?;
    if bytes.len() != N {
        anyhow::bail!(
            "Invalid hex length: expected {} bytes, got {}",
            N,
            bytes.len()
        );
    }
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to convert to array"))
}

/// Configuration for file downloads
#[derive(Clone)]
pub struct DownloadConfig {
    /// Server URL
    pub server: String,
    /// Batch ID
    pub batch_id: String,
    /// Signing key for authentication
    pub signing_key: SigningKey,
    /// Client ID
    pub client_id: String,
    /// Data directory
    pub data_dir: PathBuf,
}

/// Handles file downloads and verification
pub struct FileDownloader {
    server: String,
    batch_id: String,
    signing_key: SigningKey,
    client_id: String,
    data_dir: PathBuf,
}

impl FileDownloader {
    /// Create a new file downloader
    pub fn new(
        server: String,
        batch_id: String,
        signing_key: SigningKey,
        client_id: String,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            server,
            batch_id,
            signing_key,
            client_id,
            data_dir,
        }
    }

    /// Download and verify a file from the server
    pub fn download_and_verify(
        &self,
        filename: &str,
        root_hash: &str,
        output_dir: Option<&PathBuf>,
    ) -> Result<()> {
        // Request file hash, content, and proof from server
        let result = self.request_file_proof(filename)?;

        // Verify filename matches
        anyhow::ensure!(
            result.filename == filename,
            "Filename mismatch: expected {}, got {}",
            filename,
            result.filename
        );

        // Print received data
        self.print_received_proof(&result);

        // Verify Merkle proof
        self.verify_merkle_proof(&result, root_hash)?;

        // Decode and save file content
        let file_content = STANDARD
            .decode(&result.file_content)
            .context("Failed to decode file content from server")?;

        // Verify file hash matches downloaded content
        let downloaded_hash_hex = hex::encode(hash_leaf(&file_content));
        anyhow::ensure!(
            downloaded_hash_hex == result.file_hash,
            "File hash mismatch: expected {}, got {}",
            result.file_hash,
            downloaded_hash_hex
        );

        // Save file to output directory
        self.save_downloaded_file(&result.filename, &file_content, output_dir)?;

        println!("\n✓ File verification successful!");
        println!("  File: {}", filename);
        println!("  File hash: {}", result.file_hash);
        println!("  Verified against root: {}", root_hash);

        Ok(())
    }

    /// Request file hash and Merkle proof from server
    fn request_file_proof(&self, filename: &str) -> Result<DownloadResponse> {
        // Create message to sign
        let timestamp = self.get_current_timestamp();
        let message = self.build_download_message(filename, timestamp);

        // Sign message
        let signature = sign_message(&self.signing_key, &message);
        let signature_hex = hex::encode(signature.to_bytes());

        // Send request
        let client = Client::new();
        let url = format!("{}{}", self.server, DOWNLOAD_ENDPOINT);
        let response = client
            .get(&url)
            .query(&[
                ("filename", filename),
                ("batch_id", &self.batch_id),
                ("signature", &signature_hex),
                ("timestamp", &timestamp.to_string()),
                ("client_id", &self.client_id),
            ])
            .send()
            .context("Failed to connect to server")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Download failed: {} - {}", status, error_text);
        }

        let result: DownloadResponse = response.json()?;
        Ok(result)
    }

    /// Verify Merkle proof against stored root hash
    fn verify_merkle_proof(&self, result: &DownloadResponse, root_hash: &str) -> Result<()> {
        // Decode file hash (leaf hash)
        let leaf_hash = hex_decode_array::<32>(&result.file_hash)
            .context("Failed to decode file hash from server")?;

        // Convert proof to merkle-tree format
        let proof_nodes = self.convert_proof_to_nodes(&result.merkle_proof)?;

        // Create MerkleProof and compute root
        let proof = MerkleProof {
            leaf_index: 0, // Not used in compute_root()
            leaf_hash,
            path: proof_nodes,
        };
        let computed_root = proof
            .compute_root()
            .context("Failed to compute root from proof")?;

        // Decode expected root hash
        let expected_root =
            hex_decode_array::<32>(root_hash).context("Failed to decode root_hash")?;

        // Print verification result
        println!("\n=== Verification ===");
        println!("Computed root: {}", hex::encode(computed_root));
        println!("Expected root: {}", root_hash);

        // Verify roots match
        anyhow::ensure!(
            computed_root == expected_root,
            "✗ Verification failed: Root mismatch"
        );

        println!("✓ Verified: Root matches!");
        Ok(())
    }

    /// Convert ProofNodeJson to merkle-tree ProofNode
    fn convert_proof_to_nodes(
        &self,
        proof_json: &[ProofNodeJson],
    ) -> Result<Vec<merkle_tree::ProofNode>> {
        proof_json
            .iter()
            .map(|p| {
                let hash =
                    hex_decode_array::<32>(&p.hash).context("Failed to decode proof hash")?;
                Ok(merkle_tree::ProofNode {
                    hash,
                    is_left: p.is_left,
                })
            })
            .collect()
    }

    /// Print received proof information
    fn print_received_proof(&self, result: &DownloadResponse) {
        println!("\n=== Received from Server ===");
        println!("File hash (leaf): {}", result.file_hash);
        println!(
            "Merkle proof: {} nodes (from leaf to root)",
            result.merkle_proof.len()
        );
        if result.merkle_proof.is_empty() {
            println!("  (Empty proof - single file uploaded, file is the root)");
        } else {
            for (i, node) in result.merkle_proof.iter().enumerate() {
                let position = if node.is_left { "L" } else { "R" };
                let level_desc = if i == 0 {
                    "sibling leaf"
                } else {
                    "sibling internal"
                };
                println!("  [{}] {} ({}): {}", i, position, level_desc, node.hash);
            }
        }
    }

    /// Build message for download signature
    fn build_download_message(&self, filename: &str, timestamp: u64) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(filename.as_bytes());
        message.extend_from_slice(self.batch_id.as_bytes());
        message.extend_from_slice(&timestamp.to_be_bytes());
        message
    }

    /// Get current timestamp
    fn get_current_timestamp(&self) -> u64 {
        get_current_timestamp_ms()
    }

    /// Save downloaded file to disk
    fn save_downloaded_file(
        &self,
        filename: &str,
        content: &[u8],
        output_dir: Option<&PathBuf>,
    ) -> Result<()> {
        // Determine output directory
        let output_path = if let Some(dir) = output_dir {
            dir.clone()
        } else {
            // Default: data_dir/{batch_id}/downloaded/
            self.data_dir.join(&self.batch_id).join(DOWNLOADED_DIR)
        };

        // Create output directory if it doesn't exist
        fs::create_dir_all(&output_path).context("Failed to create output directory")?;

        // Save file
        let file_path = output_path.join(filename);
        fs::write(&file_path, content).context("Failed to write downloaded file")?;

        println!("  File saved to: {:?}", file_path);
        Ok(())
    }
}

/// Download and verify a file from the server (convenience function)
pub fn download_file(
    config: &DownloadConfig,
    filename: &str,
    root_hash: &str,
    output_dir: Option<&PathBuf>,
) -> Result<()> {
    // Validate filename to prevent path traversal attacks
    file_utils::validate_filename(filename)
        .map_err(|e| anyhow::anyhow!("{}: {}", e.message(), filename))?;
    
    let downloader = FileDownloader::new(
        config.server.clone(),
        config.batch_id.clone(),
        config.signing_key.clone(),
        config.client_id.clone(),
        config.data_dir.clone(),
    );
    downloader.download_and_verify(filename, root_hash, output_dir)
}

/// Load root hash from file
pub fn load_root_hash(batch_id: &str, data_dir: &Path) -> Result<String> {
    let root_hash_file = data_dir.join(batch_id).join(ROOT_HASH_FILE);
    if !root_hash_file.exists() {
        anyhow::bail!(
            "Root hash not found for batch {}. \
            Please provide --root-hash or upload files first to generate {}",
            batch_id,
            ROOT_HASH_FILE
        );
    }
    let root_hash = fs::read_to_string(&root_hash_file)
        .with_context(|| format!("Failed to read {}", ROOT_HASH_FILE))?
        .trim()
        .to_string();
    Ok(root_hash)
}
