use crate::auth::AuthVerifier;
use crate::handlers::error::{handle_auth_error, handle_error, handle_server_error};
use crate::handlers::upload_form::UploadForm;
use crate::state::AppState;
use actix_multipart::form::MultipartForm;
use actix_web::{post, web, HttpResponse, Result as ActixResult};
use common::file_utils;
use crypto::hash_leaf;
use tracing::info;

/// Handle file upload (multipart/form-data)
#[post("/upload")]
pub async fn upload(
    form: MultipartForm<UploadForm>,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    // Extract file path before moving form
    let file_path = form.file.file.path().to_path_buf();

    // Validate form fields (length, format checks)
    form.validate_fields()
        .map_err(actix_web::error::ErrorBadRequest)?;

    // Extract all fields from multipart form
    let UploadForm {
        file: _file,
        filename,
        batch_id,
        file_hash,
        signature,
        timestamp,
        public_key,
    } = form.into_inner();

    let filename = filename.into_inner();
    let batch_id = batch_id.into_inner();
    let file_hash = file_hash.into_inner();
    let signature_hex = signature.into_inner();
    let timestamp = timestamp.into_inner();
    let public_key_hex = public_key.into_inner();

    // Use structured logging with Debug formatter (?), which automatically escapes control characters
    info!(
        filename = ?filename,
        batch_id = ?batch_id,
        "POST /upload - Request received"
    );

    // Validate filename to prevent path traversal attacks
    file_utils::validate_filename(&filename)
        .map_err(|e| actix_web::error::ErrorBadRequest(e.message()))?;

    // Validate timestamp to prevent replay attacks
    AuthVerifier::validate_timestamp_default(timestamp)
        .map_err(|e| handle_auth_error("Timestamp validation failed", e))?;

    // Read file content from temp file
    // Note: File size is already limited by #[multipart(limit = "10MB")] in UploadForm
    let file_content =
        std::fs::read(&file_path).map_err(|e| handle_error("Failed to read uploaded file", e))?;

    // Verify file hash matches content
    let computed_hash = hash_leaf(&file_content);
    let computed_hash_hex = hex::encode(computed_hash);
    if computed_hash_hex != file_hash {
        return Err(actix_web::error::ErrorBadRequest(format!(
            "File hash mismatch: expected {}, got {}",
            file_hash, computed_hash_hex
        )));
    }

    // Build message using raw file bytes (same format as before)
    let message = build_message(&filename, &batch_id, &file_hash, &file_content, timestamp);
    let signature = AuthVerifier::parse_signature(&signature_hex)
        .map_err(|e| handle_error("Failed to parse signature", e))?;

    // Validate public key format before verification
    AuthVerifier::validate_public_key(&public_key_hex)
        .map_err(|e| handle_auth_error("Invalid public key", e))?;

    let (client_id, is_new_client) =
        AuthVerifier::verify_request_signature(&state, &message, &signature, &public_key_hex)
            .await
            .map_err(|e| handle_auth_error("Signature verification failed", e))?;

    if is_new_client {
        info!("POST /upload - Registered new client: {}", client_id);
    }

    info!(
        "POST /upload - Signature verified for client: {}",
        client_id
    );

    // Store file and metadata atomically
    state
        .storage
        .store_file_with_metadata(&client_id, &batch_id, &filename, &file_content)
        .await
        .map_err(|e| handle_server_error("Failed to store file and metadata", e))?;

    info!(
        filename = ?filename,
        client_id = ?client_id,
        batch_id = ?batch_id,
        "POST /upload - File uploaded"
    );

    Ok(HttpResponse::Ok().finish())
}

/// Build message for upload signature verification
/// Signs raw file bytes
fn build_message(
    filename: &str,
    batch_id: &str,
    file_hash: &str,
    file_content: &[u8],
    timestamp: u64,
) -> Vec<u8> {
    let mut message = Vec::new();
    message.extend_from_slice(filename.as_bytes());
    message.extend_from_slice(batch_id.as_bytes());
    message.extend_from_slice(file_hash.as_bytes());
    message.extend_from_slice(file_content);
    message.extend_from_slice(&timestamp.to_be_bytes());
    message
}
