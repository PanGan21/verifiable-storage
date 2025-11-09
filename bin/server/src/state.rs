//! Server application state management

use anyhow::{Context, Result};
use crypto::{compute_client_id, public_key_from_bytes};
use ed25519_dalek::VerifyingKey;
use hex;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::info;

const SERVER_DATA_DIR: &str = "server_data";

/// In-memory cache for public keys
pub struct AppState {
    pub public_keys: Mutex<HashMap<String, VerifyingKey>>, // client_id -> public_key
}

impl AppState {
    pub fn new() -> Self {
        Self {
            public_keys: Mutex::new(HashMap::new()),
        }
    }

    pub fn load_from_disk(&self) -> Result<()> {
        let data_dir = PathBuf::from(SERVER_DATA_DIR);
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
            return Ok(());
        }

        // Load public keys
        let mut public_keys = self.public_keys.lock().unwrap();
        for entry in fs::read_dir(&data_dir)? {
            let entry = entry?;
            let client_dir = entry.path();
            if client_dir.is_dir() {
                let public_key_file = client_dir.join("public_key.hex");
                if public_key_file.exists() {
                    let public_key_hex = fs::read_to_string(&public_key_file)
                        .context("Failed to read public key")?;
                    let public_key_bytes = hex::decode(public_key_hex.trim())
                        .context("Failed to decode public key")?;
                    let public_key = public_key_from_bytes(&public_key_bytes)
                        .context("Failed to parse public key")?;
                    let client_id = compute_client_id(&public_key);
                    public_keys.insert(client_id.clone(), public_key);
                }
            }
        }

        info!("Loaded {} public keys from disk", public_keys.len());
        Ok(())
    }
}

