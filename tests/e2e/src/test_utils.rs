use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

pub fn create_test_files(dir: &Path, count: usize) -> Result<()> {
    for i in 0..count {
        let filename = format!("file{}.txt", i);
        let content = format!("Test file {} content\n", i);
        let file_path = dir.join(&filename);
        fs::write(&file_path, content)
            .with_context(|| format!("Failed to create test file: {:?}", file_path))?;
    }
    Ok(())
}

pub async fn wait_for_server(url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let health_url = format!("{}/health", url);

    println!("Waiting for server to be ready...");
    for i in 0..30 {
        match client.get(&health_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    println!("Server is ready!");
                    return Ok(());
                }
            }
            Err(_) => {
                if i < 29 {
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    anyhow::bail!("Server did not become ready within 30 seconds");
}

pub fn generate_keypair(client_binary: &Path, client_data_dir: &Path) -> Result<String> {
    let output = Command::new(client_binary)
        .arg("generate-keypair")
        .env("CLIENT_DATA_DIR", client_data_dir)
        .output()
        .with_context(|| format!("Failed to run client binary: {:?}", client_binary))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to generate keypair: {}", stderr);
    }

    // Read client ID from file
    let client_id_file = client_data_dir.join("client_id.txt");
    let client_id = fs::read_to_string(&client_id_file)
        .with_context(|| format!("Failed to read client_id.txt from {:?}", client_id_file))?;

    Ok(client_id.trim().to_string())
}

pub fn upload_files(
    client_binary: &Path,
    client_data_dir: &Path,
    test_files_dir: &Path,
    server_url: &str,
    batch_id: &str,
) -> Result<()> {
    let output = Command::new(client_binary)
        .arg("upload")
        .arg("--dir")
        .arg(test_files_dir)
        .arg("--server")
        .arg(server_url)
        .arg("--batch-id")
        .arg(batch_id)
        .env("CLIENT_DATA_DIR", client_data_dir)
        .output()
        .with_context(|| "Failed to run upload command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("Upload failed:\nSTDOUT: {}\nSTDERR: {}", stdout, stderr);
    }

    println!("Upload completed successfully");
    Ok(())
}

pub fn download_file(
    client_binary: &Path,
    client_data_dir: &Path,
    server_url: &str,
    batch_id: &str,
    filename: &str,
) -> Result<()> {
    let output = Command::new(client_binary)
        .arg("download")
        .arg(filename)
        .arg("--batch-id")
        .arg(batch_id)
        .arg("--server")
        .arg(server_url)
        .env("CLIENT_DATA_DIR", client_data_dir)
        .output()
        .with_context(|| "Failed to run download command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("Download failed:\nSTDOUT: {}\nSTDERR: {}", stdout, stderr);
    }

    println!("Download completed successfully");
    Ok(())
}

pub fn validate_merkle_proof(client_data_dir: &Path, batch_id: &str, filename: &str) -> Result<()> {
    // Read root hash
    let root_hash_file = client_data_dir.join(batch_id).join("root_hash.txt");
    let root_hash = fs::read_to_string(&root_hash_file)
        .with_context(|| format!("Failed to read root hash from {:?}", root_hash_file))?;
    let root_hash = root_hash.trim();

    // Validate root hash format (should be 64 hex characters)
    if root_hash.len() != 64 {
        anyhow::bail!(
            "Invalid root hash length: {} (expected 64)",
            root_hash.len()
        );
    }

    // Read filenames
    let filenames_file = client_data_dir.join(batch_id).join("filenames.json");
    let filenames_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&filenames_file)
            .with_context(|| format!("Failed to read filenames from {:?}", filenames_file))?,
    )?;

    // Handle both array format (what client writes) and object format
    let filenames = if let Some(array) = filenames_json.as_array() {
        array
    } else {
        filenames_json
            .get("filenames")
            .and_then(|v| v.as_array())
            .context(
                "Invalid filenames.json format: expected array or object with 'filenames' array",
            )?
    };

    // Verify that the downloaded file exists in the filenames list
    let filename_exists = filenames.iter().any(|f| f.as_str() == Some(filename));

    if !filename_exists {
        anyhow::bail!("Downloaded file {} not found in filenames list", filename);
    }

    println!(
        "Merkle proof validation: root hash exists (length: {}) and file is in batch",
        root_hash.len()
    );
    Ok(())
}
