use std::path::PathBuf;

pub const CLIENT_DATA_DIR: &str = "client_data";
const KEY_FILE: &str = "keypair.txt";

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Default server URL
    pub server_url: String,
    /// Client data directory
    pub data_dir: PathBuf,
}

impl ClientConfig {
    /// Load configuration from environment variables or use defaults
    pub fn load() -> Self {
        let server_url = std::env::var("CLIENT_SERVER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        let data_dir = std::env::var("CLIENT_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(CLIENT_DATA_DIR));

        Self {
            server_url,
            data_dir,
        }
    }

    /// Get server URL, preferring the provided URL over the default
    pub fn get_server_url(&self, provided_url: Option<&str>) -> String {
        provided_url
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.server_url.clone())
    }
}

/// Get the path to the keypair file
pub fn get_key_file_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join(KEY_FILE)
}
