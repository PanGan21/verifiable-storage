//! HTTP request handlers

use crate::auth::AuthVerifier;
use crate::state::AppState;
use actix_web::{get, post, web, HttpResponse, Result as ActixResult};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::{DownloadRequest, DownloadResponse, HealthResponse, ProofNodeJson, UploadRequest};
use crypto::hash_leaf;
use hex;
use merkle_tree::MerkleTree;
use tracing::{error, info};

/// Helper functions for error handling
fn handle_error<E: std::fmt::Display>(msg: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorBadRequest(format!("{}: {}", msg, e))
}

fn handle_auth_error<E: std::fmt::Display>(msg: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorUnauthorized(format!("{}: {}", msg, e))
}

fn handle_server_error<E: std::fmt::Display>(msg: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorInternalServerError(format!("{}: {}", msg, e))
}

fn handle_not_found<E: std::fmt::Display>(msg: &str, id: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorNotFound(format!("Batch {} not found: {}", id, e))
}

/// Handle file upload
#[post("/upload")]
pub async fn upload(
    req: web::Json<UploadRequest>,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    info!(
        "POST /upload - Request received: filename={}, batch_id={}",
        req.filename, req.batch_id
    );

    // Verify signature
    let message = UploadHandler::build_message(&req);
    let signature = AuthVerifier::parse_signature(&req.signature)
        .map_err(|e| handle_error("Failed to parse signature", e))?;

    let client_id = AuthVerifier::verify_request_signature(&state, &message, &signature, &req.public_key)
        .await
        .map_err(|e| handle_auth_error("Signature verification failed", e))?;

    info!(
        "POST /upload - Signature verified for client: {}",
        client_id
    );

    // Decode and verify file content
    let file_content = UploadHandler::decode_and_verify_file_content(&req)?;

    // Store file and update metadata
    state.storage.store_file(&client_id, &req.batch_id, &req.filename, &file_content)
        .await
        .map_err(|e| handle_server_error("Failed to store file", e))?;

    state.storage.add_filename_to_metadata(&client_id, &req.batch_id, &req.filename)
        .await
        .map_err(|e| handle_server_error("Failed to update metadata", e))?;

    info!(
        "POST /upload - File uploaded: {} (client: {}, batch: {})",
        req.filename, client_id, req.batch_id
    );

    Ok(HttpResponse::Ok().finish())
}

/// Handle file download and proof generation
#[get("/download")]
pub async fn download(
    query: web::Query<DownloadRequest>,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let req = query.into_inner();

    info!(
        "GET /download - Request received: filename={}, batch_id={}",
        req.filename, req.batch_id
    );

    // Verify signature
    let message = DownloadHandler::build_message(&req.filename, &req.batch_id, req.timestamp);
    let signature_obj = AuthVerifier::parse_signature(&req.signature)
        .map_err(|e| handle_error("Failed to parse signature", e))?;

    let client_id = AuthVerifier::verify_request_signature_with_stored_keys(&state, &message, &signature_obj)
        .await
        .map_err(|e| handle_auth_error("Signature verification failed", e))?;

    info!(
        "GET /download - Signature verified for client: {}",
        client_id
    );

    // Generate proof for the requested file
    let file_with_proof = DownloadHandler::generate_file_proof(&state, &client_id, &req.batch_id, &req.filename).await?;

    info!(
        "GET /download - File hash and proof for {} (proof length: {})",
        req.filename,
        file_with_proof.merkle_proof.len()
    );

    Ok(HttpResponse::Ok().json(file_with_proof))
}

/// Health check endpoint
#[get("/health")]
pub async fn health() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
    }))
}

/// Helper struct for upload handler operations
pub struct UploadHandler;

impl UploadHandler {
    /// Build message for upload signature verification
    pub fn build_message(req: &UploadRequest) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(req.filename.as_bytes());
        message.extend_from_slice(req.batch_id.as_bytes());
        message.extend_from_slice(req.file_hash.as_bytes());
        message.extend_from_slice(req.file_content.as_bytes());
        message.extend_from_slice(&req.timestamp.to_be_bytes());
        message
    }

    /// Decode file content and verify hash
    pub fn decode_and_verify_file_content(req: &UploadRequest) -> ActixResult<Vec<u8>> {
        let file_content = STANDARD
            .decode(&req.file_content)
            .map_err(|e| handle_error("Failed to decode file content", e))?;

        // Verify file hash matches content
        let computed_hash = hash_leaf(&file_content);
        let computed_hash_hex = hex::encode(computed_hash);
        if computed_hash_hex != req.file_hash {
            return Err(actix_web::error::ErrorBadRequest(format!(
                "File hash mismatch: expected {}, got {}",
                req.file_hash, computed_hash_hex
            )));
        }

        Ok(file_content)
    }
}

/// Helper struct for download handler operations
pub struct DownloadHandler;

impl DownloadHandler {
    /// Build message for download signature verification
    pub fn build_message(filename: &str, batch_id: &str, timestamp: u64) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(filename.as_bytes());
        message.extend_from_slice(batch_id.as_bytes());
        message.extend_from_slice(&timestamp.to_be_bytes());
        message
    }

    /// Generate Merkle proof for a file
    pub async fn generate_file_proof(
        state: &web::Data<AppState>,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> ActixResult<DownloadResponse> {
        // Load batch metadata
        let filenames = state.storage.load_batch_filenames(client_id, batch_id)
            .await
            .map_err(|e| handle_not_found("Failed to load batch", batch_id, e))?;

        // Verify filename is in batch
        if !filenames.contains(&filename.to_string()) {
            return Err(actix_web::error::ErrorNotFound(format!(
                "File {} not found in batch {}",
                filename, batch_id
            )));
        }

        // Verify file exists
        let exists = state.storage.file_exists(client_id, batch_id, filename)
            .await
            .map_err(|e| handle_server_error("Failed to check file existence", e))?;
        
        if !exists {
            return Err(actix_web::error::ErrorNotFound(format!(
                "File {} not found in batch {} for client {}",
                filename, batch_id, client_id
            )));
        }

        // Read file and compute hash
        let file_content = state.storage.read_file(client_id, batch_id, filename)
            .await
            .map_err(|e| handle_server_error("Failed to read file", e))?;

        let file_hash = hash_leaf(&file_content);
        let file_hash_hex = hex::encode(file_hash);

        // Encode file content as base64
        let file_content_b64 = STANDARD.encode(&file_content);

        // Build Merkle tree and generate proof
        let proof = Self::build_merkle_proof(state, client_id, batch_id, &filenames, filename).await?;

        // Convert proof to JSON format
        let proof_json: Vec<ProofNodeJson> = proof
            .path
            .iter()
            .map(|p| ProofNodeJson {
                hash: hex::encode(p.hash),
                is_left: p.is_left,
            })
            .collect();

        Ok(DownloadResponse {
            filename: filename.to_string(),
            file_hash: file_hash_hex,
            file_content: file_content_b64,
            merkle_proof: proof_json,
        })
    }

    /// Build Merkle tree and generate proof for a file
    async fn build_merkle_proof(
        state: &web::Data<AppState>,
        client_id: &str,
        batch_id: &str,
        filenames: &[String],
        filename: &str,
    ) -> ActixResult<merkle_tree::MerkleProof> {
        // Sort filenames to ensure deterministic order
        let mut sorted_filenames = filenames.to_vec();
        sorted_filenames.sort();

        // Read all files in batch
        let file_data = state.storage.read_batch_files(client_id, batch_id, &sorted_filenames)
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
                actix_web::error::ErrorNotFound(format!("File {} not found", filename))
            })?;

        let proof = tree.generate_proof(file_index)
            .map_err(|e| handle_server_error("Failed to generate proof", e))?;

        Ok(proof)
    }
}
