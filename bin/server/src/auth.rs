use crate::constants::{DEFAULT_MAX_AGE_SECONDS, DEFAULT_MAX_CLOCK_SKEW_SECONDS};
use crate::state::AppState;
use actix_web::web;
use anyhow::{Context, Result};
use crypto::{compute_client_id, public_key_from_bytes, verify_signature};
use ed25519_dalek::Signature;
use std::time::{SystemTime, UNIX_EPOCH};

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

    /// Verify request signature using client_id for key lookup
    pub async fn verify_request_signature_with_client_id(
        state: &web::Data<AppState>,
        client_id: &str,
        message: &[u8],
        signature: &Signature,
    ) -> Result<()> {
        let public_key_bytes = state
            .storage
            .load_public_key(client_id)
            .await
            .context("Failed to load public key")?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Client not found: {}. Client must be registered first (e.g., by uploading files)",
                    client_id
                )
            })?;

        let public_key =
            public_key_from_bytes(&public_key_bytes).context("Failed to parse public key")?;

        verify_signature(&public_key, message, signature)
            .context("Signature verification failed")?;

        Ok(())
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

    /// Validate timestamp to prevent replay attacks
    /// Checks that the timestamp is within the allowed window (not too old, not too far in future)
    ///
    /// # Arguments
    /// * `request_timestamp_ms` - Timestamp from the request in milliseconds since Unix epoch
    /// * `max_age_seconds` - Maximum age of the request in seconds (default: 300 = 5 minutes)
    /// * `max_clock_skew_seconds` - Maximum allowed clock skew in seconds (default: 60 = 1 minute)
    ///
    /// # Returns
    /// * `Ok(())` if timestamp is valid
    /// * `Err` if timestamp is too old, too far in future, or invalid
    pub fn validate_timestamp(
        request_timestamp_ms: u64,
        max_age_seconds: u64,
        max_clock_skew_seconds: u64,
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("Failed to get current time")?
            .as_millis() as u64;

        let request_timestamp_seconds = request_timestamp_ms / 1000;
        let now_seconds = now / 1000;

        // Check if timestamp is too old
        let age_seconds = now_seconds.saturating_sub(request_timestamp_seconds);
        if age_seconds > max_age_seconds {
            anyhow::bail!(
                "Request timestamp is too old: {} seconds old (max: {} seconds)",
                age_seconds,
                max_age_seconds
            );
        }

        // Check if timestamp is too far in the future (clock skew)
        if request_timestamp_seconds > now_seconds + max_clock_skew_seconds {
            anyhow::bail!(
                "Request timestamp is too far in the future: {} seconds ahead (max clock skew: {} seconds)",
                request_timestamp_seconds.saturating_sub(now_seconds),
                max_clock_skew_seconds
            );
        }

        Ok(())
    }

    /// Validate timestamp with default settings (5 minutes max age, 1 minute clock skew)
    pub fn validate_timestamp_default(request_timestamp_ms: u64) -> Result<()> {
        Self::validate_timestamp(
            request_timestamp_ms,
            DEFAULT_MAX_AGE_SECONDS,
            DEFAULT_MAX_CLOCK_SKEW_SECONDS,
        )
    }
}
