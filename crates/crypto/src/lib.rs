use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Generate a new key pair
pub fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

/// Compute Client ID from public key: SHA256(public_key)
pub fn compute_client_id(public_key: &VerifyingKey) -> String {
    hex::encode(Sha256::digest(public_key.as_bytes()))
}

/// Load or generate keypair from file
pub fn load_or_generate_keypair(key_file: &Path) -> Result<(SigningKey, VerifyingKey, String)> {
    if key_file.exists() {
        // Load existing keypair
        let key_data = fs::read_to_string(key_file).context("Failed to read key file")?;

        // Parse as hex-encoded secret key (64 bytes: 32-byte seed + 32-byte public key)
        let key_bytes = hex::decode(key_data.trim()).context("Failed to decode key file")?;

        if key_bytes.len() != 64 {
            anyhow::bail!(
                "Invalid key file format. Expected 64 bytes, got {}",
                key_bytes.len()
            );
        }

        // Extract secret key (first 32 bytes) and reconstruct keypair
        let mut secret_key_bytes = [0u8; 32];
        secret_key_bytes.copy_from_slice(&key_bytes[0..32]);
        let signing_key = SigningKey::from_bytes(&secret_key_bytes);
        let verifying_key = signing_key.verifying_key();

        let client_id = compute_client_id(&verifying_key);
        Ok((signing_key, verifying_key, client_id))
    } else {
        // Generate new keypair
        let (signing_key, verifying_key) = generate_keypair();
        let client_id = compute_client_id(&verifying_key);

        // Save keypair (store secret key + public key = 64 bytes)
        let mut key_data = Vec::with_capacity(64);
        key_data.extend_from_slice(signing_key.as_bytes());
        key_data.extend_from_slice(verifying_key.as_bytes());

        if let Some(parent) = key_file.parent() {
            fs::create_dir_all(parent).context("Failed to create key directory")?;
        }

        fs::write(key_file, hex::encode(key_data)).context("Failed to write key file")?;

        Ok((signing_key, verifying_key, client_id))
    }
}

/// Sign a message with the signing key
pub fn sign_message(signing_key: &SigningKey, message: &[u8]) -> Signature {
    signing_key.sign(message)
}

/// Verify a signature
pub fn verify_signature(
    verifying_key: &VerifyingKey,
    message: &[u8],
    signature: &Signature,
) -> Result<()> {
    verifying_key
        .verify(message, signature)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))
}

/// Deserialize public key from bytes
pub fn public_key_from_bytes(bytes: &[u8]) -> Result<VerifyingKey> {
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;
    VerifyingKey::from_bytes(&array).map_err(|e| anyhow::anyhow!("Invalid public key: {}", e))
}

/// Compute file hash for Merkle tree leaf: hash_leaf(file_content)
pub fn hash_leaf(data: &[u8]) -> [u8; 32] {
    Sha256::new()
        .chain_update([0x00]) // Domain separation prefix for leaves
        .chain_update(data)
        .finalize()
        .into()
}
