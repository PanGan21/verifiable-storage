//! Client configuration constants

use std::path::PathBuf;

pub const CLIENT_DATA_DIR: &str = "client_data";
const KEY_FILE: &str = "keypair.txt";

/// Get the path to the keypair file
pub fn get_key_file_path() -> PathBuf {
    PathBuf::from(CLIENT_DATA_DIR).join(KEY_FILE)
}

