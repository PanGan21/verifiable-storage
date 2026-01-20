pub mod file_utils;
pub mod utils;

use serde::{Deserialize, Serialize};

/// Request to download a file from the server (query parameters)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DownloadRequest {
    pub filename: String,  // Original filename
    pub batch_id: String,  // Batch ID this file belongs to
    pub signature: String, // hex-encoded signature
    pub timestamp: u64,    // Timestamp for replay attack prevention
    pub client_id: String, // Client ID (SHA256 hash of public key) for O(1) key lookup
}

/// Download response containing file data and Merkle proof
/// Note: Server returns file content and proof - not the root hash
/// Client computes file hash from content and verifies proof against stored root hash
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DownloadResponse {
    pub filename: String,     // Original filename
    pub file_content: String, // base64-encoded file content
    pub merkle_proof: Vec<ProofNodeJson>,
}

/// JSON representation of a Merkle proof node
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProofNodeJson {
    pub hash: String, // hex-encoded
    pub is_left: bool,
}

/// Response from health check endpoint
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthResponse {
    pub status: String, // "ok" when healthy
}
