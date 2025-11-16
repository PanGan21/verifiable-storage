use actix_multipart::form::{tempfile::TempFile, text::Text, MultipartForm};

/// Multipart form for file upload
#[derive(MultipartForm)]
pub struct UploadForm {
    /// The file being uploaded
    #[multipart(limit = "10MB")]
    pub file: TempFile,

    /// Original filename
    pub filename: Text<String>,

    /// Batch ID this file belongs to
    pub batch_id: Text<String>,

    /// Hex-encoded leaf hash of the file
    pub file_hash: Text<String>,

    /// Hex-encoded Ed25519 signature
    pub signature: Text<String>,

    /// Timestamp in milliseconds since Unix epoch
    pub timestamp: Text<u64>,

    /// Hex-encoded Ed25519 public key
    pub public_key: Text<String>,
}

impl UploadForm {
    /// Validate form fields
    pub fn validate_fields(&self) -> Result<(), String> {
        let filename = &self.filename.0;
        let batch_id = &self.batch_id.0;
        let file_hash = &self.file_hash.0;
        let signature = &self.signature.0;
        let public_key = &self.public_key.0;

        if filename.is_empty() || filename.len() > 255 {
            return Err("Filename must be between 1 and 255 characters".to_string());
        }

        if batch_id.is_empty() || batch_id.len() > 255 {
            return Err("Batch ID must be between 1 and 255 characters".to_string());
        }

        if file_hash.len() != 64 {
            return Err("File hash must be exactly 64 hex characters".to_string());
        }

        if signature.len() != 128 {
            return Err("Signature must be exactly 128 hex characters".to_string());
        }

        if public_key.len() != 64 {
            return Err("Public key must be exactly 64 hex characters".to_string());
        }

        Ok(())
    }
}
