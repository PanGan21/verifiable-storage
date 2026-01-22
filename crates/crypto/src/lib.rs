use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm,
};
use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
#[allow(deprecated)] // generic-array 0.14 API is deprecated but required by aes-gcm 0.10
use generic_array::{typenum::U12, GenericArray};
use hkdf::Hkdf;
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
        let key_data = fs::read_to_string(key_file).context("Failed to read key file")?;

        // Parse as hex-encoded key (64 bytes: 32-byte secret + 32-byte public)
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

        // Store secret key + public key = 64 bytes
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

/// Compute file hash for Merkle tree leaf with domain separation prefix
pub fn hash_leaf(data: &[u8]) -> [u8; 32] {
    Sha256::new()
        .chain_update([0x00]) // Domain separation prefix for leaves
        .chain_update(data)
        .finalize()
        .into()
}

/// Derive encryption key from Ed25519 signing key using HKDF
/// Uses a fixed salt to ensure deterministic key derivation
fn derive_encryption_key(signing_key: &SigningKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, signing_key.as_bytes());
    // output key material
    let mut okm = [0u8; 32];
    // expand the key material with domain separation prefix
    hk.expand(b"verifiable-storage-encryption-key", &mut okm)
        .expect("HKDF expansion failed");
    okm
}

/// Derive a deterministic nonce for file encryption
/// Uses filename and batch_id to ensure unique nonce per file
#[allow(deprecated)] // generic-array 0.14 API is deprecated but required by aes-gcm 0.10
fn derive_nonce(filename: &str, batch_id: &str) -> GenericArray<u8, U12> {
    let mut hasher = Sha256::new();
    hasher.update(b"verifiable-storage-nonce");
    hasher.update(filename.as_bytes());
    hasher.update(batch_id.as_bytes());
    let hash = hasher.finalize();

    // Use first 12 bytes of hash as nonce (AES-GCM requires 12-byte nonce)
    // Convert [u8; 12] to GenericArray using TryInto (non-deprecated path)
    let nonce_bytes: [u8; 12] = hash[..12].try_into().expect("Hash slice is 12 bytes");
    nonce_bytes.into()
}

/// Encrypt file content using AES-256-GCM
/// Derives encryption key from Ed25519 signing key
/// Uses deterministic nonce based on filename and batch_id
#[allow(deprecated)] // generic-array 0.14 API is deprecated but required by aes-gcm 0.10
pub fn encrypt_file(
    signing_key: &SigningKey,
    filename: &str,
    batch_id: &str,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let key = derive_encryption_key(signing_key);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("Failed to create cipher: {}", e))?;
    let nonce = derive_nonce(filename, batch_id);

    cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))
}

/// Decrypt file content using AES-256-GCM
/// Derives encryption key from Ed25519 signing key
/// Uses deterministic nonce based on filename and batch_id
#[allow(deprecated)] // generic-array 0.14 API is deprecated but required by aes-gcm 0.10
pub fn decrypt_file(
    signing_key: &SigningKey,
    filename: &str,
    batch_id: &str,
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    let key = derive_encryption_key(signing_key);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("Failed to create cipher: {}", e))?;
    let nonce = derive_nonce(filename, batch_id);

    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))
}
