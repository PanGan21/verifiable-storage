use crate::config::get_key_file_path;
use anyhow::{Context, Result};
use crypto::{compute_client_id, generate_keypair};
use hex;
use log::info;
use std::fs;
use std::path::PathBuf;

/// Manages keypair generation and loading
pub struct KeypairManager;

impl KeypairManager {
    /// Generate a new keypair
    pub fn generate_keypair(data_dir: &PathBuf, force: bool) -> Result<()> {
        fs::create_dir_all(data_dir).context("Failed to create client_data directory")?;

        let key_file = get_key_file_path(data_dir);

        if key_file.exists() && !force {
            anyhow::bail!(
                "Keypair already exists at {:?}. Use --force to overwrite it.",
                key_file
            );
        }

        // If force is true, remove existing keypair
        if force && key_file.exists() {
            Self::remove_existing_keypair(data_dir, &key_file)?;
            info!("Removed existing keypair");
        }

        // Generate new keypair
        let (signing_key, verifying_key) = generate_keypair();
        let client_id = compute_client_id(&verifying_key);

        // Save keypair and client ID
        Self::save_keypair(&key_file, &signing_key, &verifying_key)?;
        Self::save_client_id(data_dir, &client_id)?;

        info!("Generated new keypair");
        info!("Client ID: {}", client_id);
        println!("✓ Keypair generated successfully");
        println!("Client ID: {}", client_id);
        println!("Keypair saved to: {:?}", key_file);

        if force {
            println!("⚠️  Warning: Existing keypair was overwritten. You will need to re-register with the server.");
        }

        Ok(())
    }

    /// Get or create keypair
    pub fn get_or_create_keypair(
        data_dir: &PathBuf,
    ) -> Result<(ed25519_dalek::SigningKey, String)> {
        fs::create_dir_all(data_dir).context("Failed to create client_data directory")?;
        let key_file = get_key_file_path(data_dir);
        let (signing_key, _verifying_key, client_id) = crypto::load_or_generate_keypair(&key_file)?;
        Ok((signing_key, client_id))
    }

    /// Remove existing keypair files
    fn remove_existing_keypair(data_dir: &PathBuf, key_file: &PathBuf) -> Result<()> {
        fs::remove_file(key_file)?;
        let client_id_file = data_dir.join("client_id.txt");
        if client_id_file.exists() {
            fs::remove_file(&client_id_file)?;
        }
        Ok(())
    }

    /// Save keypair to file
    fn save_keypair(
        key_file: &PathBuf,
        signing_key: &ed25519_dalek::SigningKey,
        verifying_key: &ed25519_dalek::VerifyingKey,
    ) -> Result<()> {
        // Save keypair (64 bytes: 32-byte secret key + 32-byte public key)
        let mut key_data = Vec::with_capacity(64);
        key_data.extend_from_slice(signing_key.as_bytes());
        key_data.extend_from_slice(verifying_key.to_bytes().as_slice());

        // Ensure directory exists
        if let Some(parent) = key_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write keypair
        fs::write(key_file, hex::encode(key_data))?;
        Ok(())
    }

    /// Save client ID to file
    fn save_client_id(data_dir: &PathBuf, client_id: &str) -> Result<()> {
        let client_id_file = data_dir.join("client_id.txt");
        if let Some(parent) = client_id_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&client_id_file, client_id)?;
        Ok(())
    }
}

/// Generate keypair command (convenience function)
pub fn generate_keypair_command(data_dir: &PathBuf, force: bool) -> Result<()> {
    KeypairManager::generate_keypair(data_dir, force)
}

/// Get or create keypair (convenience function)
pub fn get_or_create_keypair(data_dir: &PathBuf) -> Result<(ed25519_dalek::SigningKey, String)> {
    KeypairManager::get_or_create_keypair(data_dir)
}
