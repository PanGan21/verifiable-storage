use crate::auth::AuthVerifier;
use crate::handlers::error::{
    handle_auth_error, handle_error, handle_not_found, handle_server_error,
};
use crate::handlers::proof::{generate_proof, prepare_file_data, proof_to_json};
use crate::state::AppState;
use actix_web::{get, web, HttpResponse, Result as ActixResult};
use common::{DownloadRequest, DownloadResponse};
use tracing::info;

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
    let message = build_message(&req.filename, &req.batch_id, req.timestamp);
    let signature_obj = AuthVerifier::parse_signature(&req.signature)
        .map_err(|e| handle_error("Failed to parse signature", e))?;

    let client_id =
        AuthVerifier::verify_request_signature_with_stored_keys(&state, &message, &signature_obj)
            .await
            .map_err(|e| handle_auth_error("Signature verification failed", e))?;

    info!(
        "GET /download - Signature verified for client: {}",
        client_id
    );

    // Load batch metadata and verify file exists
    let filenames = state
        .storage
        .load_batch_filenames(&client_id, &req.batch_id)
        .await
        .map_err(|e| handle_not_found("Failed to load batch", &req.batch_id, e))?;

    verify_file_in_batch(&filenames, &req.filename, &req.batch_id)?;

    // Verify file exists in storage
    let exists = state
        .storage
        .file_exists(&client_id, &req.batch_id, &req.filename)
        .await
        .map_err(|e| handle_server_error("Failed to check file existence", e))?;

    if !exists {
        return Err(actix_web::error::ErrorNotFound(format!(
            "File {} not found in batch {} for client {}",
            req.filename, req.batch_id, client_id
        )));
    }

    // Read file and prepare response data
    let file_content = state
        .storage
        .read_file(&client_id, &req.batch_id, &req.filename)
        .await
        .map_err(|e| handle_server_error("Failed to read file", e))?;

    let (file_hash_hex, file_content_b64) = prepare_file_data(&file_content);

    // Generate Merkle proof
    let proof =
        generate_proof(&state, &client_id, &req.batch_id, &filenames, &req.filename).await?;
    let proof_json = proof_to_json(&proof);

    info!(
        "GET /download - File hash and proof for {} (proof length: {})",
        req.filename,
        proof_json.len()
    );

    Ok(HttpResponse::Ok().json(DownloadResponse {
        filename: req.filename,
        file_hash: file_hash_hex,
        file_content: file_content_b64,
        merkle_proof: proof_json,
    }))
}

/// Build message for download signature verification
fn build_message(filename: &str, batch_id: &str, timestamp: u64) -> Vec<u8> {
    let mut message = Vec::new();
    message.extend_from_slice(filename.as_bytes());
    message.extend_from_slice(batch_id.as_bytes());
    message.extend_from_slice(&timestamp.to_be_bytes());
    message
}

/// Verify that filename exists in batch
fn verify_file_in_batch(filenames: &[String], filename: &str, batch_id: &str) -> ActixResult<()> {
    if !filenames.contains(&filename.to_string()) {
        return Err(actix_web::error::ErrorNotFound(format!(
            "File {} not found in batch {}",
            filename, batch_id
        )));
    }
    Ok(())
}
