use crate::state::AppState;
use actix_web::web;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::ProofNodeJson;
use crypto::hash_leaf;
use merkle_tree::MerkleTree;
use tracing::error;

use crate::handlers::error::handle_server_error;

/// Generate Merkle proof for a file in a batch
pub async fn generate_proof(
    state: &web::Data<AppState>,
    client_id: &str,
    batch_id: &str,
    filenames: &[String],
    filename: &str,
) -> Result<merkle_tree::MerkleProof, actix_web::Error> {
    // Sort filenames to ensure deterministic order
    let mut sorted_filenames = filenames.to_vec();
    sorted_filenames.sort();

    // Read all files in batch
    let file_data = state
        .storage
        .read_batch_files(client_id, batch_id, &sorted_filenames)
        .await
        .map_err(|e| handle_server_error("Failed to read batch files", e))?;

    // Build Merkle tree
    let tree = MerkleTree::from_data(&file_data)
        .map_err(|e| handle_server_error("Failed to build Merkle tree", e))?;

    // Find file index and generate proof
    let file_index = sorted_filenames
        .iter()
        .position(|name| name == filename)
        .ok_or_else(|| {
            error!("File {} not found in sorted filenames", filename);
            actix_web::error::ErrorNotFound(format!("File {} not found", filename))
        })?;

    tree.generate_proof(file_index)
        .map_err(|e| handle_server_error("Failed to generate proof", e))
}

/// Convert Merkle proof to JSON format
pub fn proof_to_json(proof: &merkle_tree::MerkleProof) -> Vec<ProofNodeJson> {
    proof
        .path
        .iter()
        .map(|p| ProofNodeJson {
            hash: hex::encode(p.hash),
            is_left: p.is_left,
        })
        .collect()
}

/// Prepare file data for download response
pub fn prepare_file_data(file_content: &[u8]) -> (String, String) {
    let file_hash = hash_leaf(file_content);
    let file_hash_hex = hex::encode(file_hash);
    let file_content_b64 = STANDARD.encode(file_content);
    (file_hash_hex, file_content_b64)
}

