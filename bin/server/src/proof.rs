use crate::state::AppState;
use actix_web::web;
use common::ProofNodeJson;
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

    // Load stored Merkle tree from database/filesystem
    let tree = state
        .storage
        .load_merkle_tree(client_id, batch_id)
        .await
        .map_err(|e| handle_server_error("Failed to load Merkle tree", e))?
        .ok_or_else(|| {
            error!("Merkle tree not found in storage for batch {}", batch_id);
            handle_server_error(
                "Merkle tree not found",
                anyhow::anyhow!(
                    "Merkle tree not found for batch {} - batch may not have been uploaded with tree storage enabled",
                    batch_id
                ),
            )
        })?;

    // Verify stored tree has correct number of leaves (data integrity check)
    if tree.num_leaves() != sorted_filenames.len() {
        error!(
            "Stored tree has {} leaves but batch has {} files - tree is out of sync",
            tree.num_leaves(),
            sorted_filenames.len()
        );
        return Err(handle_server_error(
            "Stored Merkle tree is invalid (leaf count mismatch)",
            anyhow::anyhow!(
                "Tree has {} leaves but batch has {} files",
                tree.num_leaves(),
                sorted_filenames.len()
            ),
        ));
    }

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
