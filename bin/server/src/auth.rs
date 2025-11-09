//! Authentication and signature verification

use crate::state::AppState;
use actix_web::web;
use anyhow::{Context, Result};
use crypto::{compute_client_id, public_key_from_bytes, verify_signature};
use ed25519_dalek::Signature;
use hex;
use std::fs;
use std::path::PathBuf;
use tracing::info;

const SERVER_DATA_DIR: &str = "server_data";

/// Handles authentication and signature verification
pub struct AuthVerifier;

impl AuthVerifier {
    /// Verify request signature and auto-register client if needed
    pub fn verify_request_signature(
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
        Self::get_or_create_client(state, &client_id, &public_key_bytes, &public_key)?;

        Ok(client_id)
    }

    /// Verify request signature using stored public keys
    pub fn verify_request_signature_with_stored_keys(
        state: &web::Data<AppState>,
        message: &[u8],
        signature: &Signature,
    ) -> Result<String> {
        // Try all stored public keys to find the one that verifies the signature
        let public_keys = state.public_keys.lock().unwrap();
        for (client_id, public_key) in public_keys.iter() {
            if verify_signature(public_key, message, signature).is_ok() {
                return Ok(client_id.clone());
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
    fn get_or_create_client(
        state: &web::Data<AppState>,
        client_id: &str,
        public_key_bytes: &[u8],
        public_key: &ed25519_dalek::VerifyingKey,
    ) -> Result<()> {
        let data_dir = PathBuf::from(SERVER_DATA_DIR).join(client_id);
        let public_key_file = data_dir.join("public_key.hex");

        if !public_key_file.exists() {
            // Register: store public key
            fs::create_dir_all(&data_dir).context("Failed to create client directory")?;
            fs::write(&public_key_file, hex::encode(public_key_bytes))
                .context("Failed to write public key")?;

            // Update in-memory cache
            let mut public_keys = state.public_keys.lock().unwrap();
            public_keys.insert(client_id.to_string(), *public_key);

            info!("POST /upload - Registered new client: {}", client_id);
        } else {
            // Client already registered, ensure it's in memory cache
            let mut public_keys = state.public_keys.lock().unwrap();
            if !public_keys.contains_key(client_id) {
                public_keys.insert(client_id.to_string(), *public_key);
            }
        }

        Ok(())
    }
}
