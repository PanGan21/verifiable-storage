//! Authentication and signature verification

use crate::state::AppState;
use actix_web::web;
use anyhow::{Context, Result};
use crypto::{compute_client_id, public_key_from_bytes, verify_signature};
use ed25519_dalek::Signature;
use hex;
use tracing::info;

/// Handles authentication and signature verification
pub struct AuthVerifier;

impl AuthVerifier {
    /// Verify request signature and auto-register client if needed
    pub async fn verify_request_signature(
        state: &web::Data<AppState>,
        message: &[u8],
        signature: &Signature,
        public_key_hex: &str,
    ) -> Result<String> {
        // Decode public key from request
        let public_key_bytes =
            hex::decode(public_key_hex.trim()).context("Failed to decode public key")?;
        let public_key =
            public_key_from_bytes(&public_key_bytes).context("Failed to parse public key")?;

        // Compute client ID
        let client_id = compute_client_id(&public_key);

        // Verify signature
        verify_signature(&public_key, message, signature)
            .context("Signature verification failed")?;

        // Check if client is registered, if not, auto-register
        Self::get_or_create_client(state, &client_id, &public_key_bytes, &public_key).await?;

        Ok(client_id)
    }

    /// Verify request signature using stored public keys
    /// Fetches public keys on-demand from storage
    pub async fn verify_request_signature_with_stored_keys(
        state: &web::Data<AppState>,
        message: &[u8],
        signature: &Signature,
    ) -> Result<String> {
        // Get all client IDs from storage
        let client_ids = state.storage.list_client_ids().await
            .context("Failed to list client IDs")?;

        // Try each client's public key to find the one that verifies the signature
        for client_id in client_ids {
            if let Some(public_key_bytes) = state.storage.load_public_key(&client_id).await
                .context("Failed to load public key")? {
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

    /// Ensure client is registered (auto-register if needed)
    async fn get_or_create_client(
        state: &web::Data<AppState>,
        client_id: &str,
        public_key_bytes: &[u8],
        _public_key: &ed25519_dalek::VerifyingKey,
    ) -> Result<()> {
        // Check if public key exists in storage
        let existing_key = state.storage.load_public_key(client_id).await?;

        if existing_key.is_none() {
            // Register: store public key
            state
                .storage
                .store_public_key(client_id, public_key_bytes)
                .await
                .context("Failed to store public key")?;

            info!("POST /upload - Registered new client: {}", client_id);
        }

        Ok(())
    }
}
