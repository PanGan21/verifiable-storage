use crate::auth::AuthVerifier;
use crate::handlers::error::{handle_auth_error, handle_error, handle_server_error};
use crate::state::AppState;
use actix_web::{post, web, HttpResponse, Result as ActixResult};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use common::UploadRequest;
use crypto::hash_leaf;
use tracing::info;

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
    let message = build_message(&req);
    let signature = AuthVerifier::parse_signature(&req.signature)
        .map_err(|e| handle_error("Failed to parse signature", e))?;

    let client_id =
        AuthVerifier::verify_request_signature(&state, &message, &signature, &req.public_key)
            .await
            .map_err(|e| handle_auth_error("Signature verification failed", e))?;

    info!(
        "POST /upload - Signature verified for client: {}",
        client_id
    );

    // Decode and verify file content
    let file_content = decode_and_verify_file_content(&req)?;

    // Store file and update metadata
    state
        .storage
        .store_file(&client_id, &req.batch_id, &req.filename, &file_content)
        .await
        .map_err(|e| handle_server_error("Failed to store file", e))?;

    state
        .storage
        .add_filename_to_metadata(&client_id, &req.batch_id, &req.filename)
        .await
        .map_err(|e| handle_server_error("Failed to update metadata", e))?;

    info!(
        "POST /upload - File uploaded: {} (client: {}, batch: {})",
        req.filename, client_id, req.batch_id
    );

    Ok(HttpResponse::Ok().finish())
}

/// Build message for upload signature verification
fn build_message(req: &UploadRequest) -> Vec<u8> {
    let mut message = Vec::new();
    message.extend_from_slice(req.filename.as_bytes());
    message.extend_from_slice(req.batch_id.as_bytes());
    message.extend_from_slice(req.file_hash.as_bytes());
    message.extend_from_slice(req.file_content.as_bytes());
    message.extend_from_slice(&req.timestamp.to_be_bytes());
    message
}

/// Decode file content and verify hash
fn decode_and_verify_file_content(req: &UploadRequest) -> ActixResult<Vec<u8>> {
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

/// Health check endpoint
#[actix_web::get("/health")]
pub async fn health() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(common::HealthResponse {
        status: "ok".to_string(),
    }))
}
