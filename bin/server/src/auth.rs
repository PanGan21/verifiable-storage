use crate::state::AppState;
use actix_web::web;
use anyhow::{Context, Result};
use crypto::{compute_client_id, public_key_from_bytes, verify_signature};
use ed25519_dalek::Signature;

/// Handles authentication and signature verification
pub struct AuthVerifier;

impl AuthVerifier {
    pub async fn verify_request_signature(
        state: &web::Data<AppState>,
        message: &[u8],
        signature: &Signature,
        public_key_hex: &str,
    ) -> Result<(String, bool)> {
        let public_key_bytes =
            hex::decode(public_key_hex.trim()).context("Failed to decode public key")?;
        let public_key =
            public_key_from_bytes(&public_key_bytes).context("Failed to parse public key")?;

        let client_id = compute_client_id(&public_key);

        verify_signature(&public_key, message, signature)
            .context("Signature verification failed")?;

        let is_new = state
            .storage
            .load_public_key(&client_id)
            .await
            .context("Failed to check if client exists")?
            .is_none();

        if is_new {
            state
                .storage
                .store_public_key(&client_id, &public_key_bytes)
                .await
                .context("Failed to store public key")?;
        }

        Ok((client_id, is_new))
    }

    pub async fn verify_request_signature_with_stored_keys(
        state: &web::Data<AppState>,
        message: &[u8],
        signature: &Signature,
    ) -> Result<String> {
        let client_ids = state
            .storage
            .list_client_ids()
            .await
            .context("Failed to list client IDs")?;

        // Try each client's public key to find the one that verifies the signature
        for client_id in client_ids {
            if let Some(public_key_bytes) = state
                .storage
                .load_public_key(&client_id)
                .await
                .context("Failed to load public key")?
            {
                let public_key = public_key_from_bytes(&public_key_bytes)
                    .context("Failed to parse public key")?;
                if verify_signature(&public_key, message, signature).is_ok() {
                    return Ok(client_id);
                }
            }
        }
        anyhow::bail!("Signature verification failed: no matching public key found");
    }

    /// Parse signature from hex string
    pub fn parse_signature(signature_hex: &str) -> Result<Signature> {
        let signature_bytes =
            hex::decode(signature_hex.trim()).context("Failed to decode signature")?;
        let signature_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid signature length"))?;
        Ok(Signature::from_bytes(&signature_array))
    }
}
