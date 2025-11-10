use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn validate_upload(
    server_data_dir: &Path,
    client_id: &str,
    batch_id: &str,
    expected_file_count: usize,
) -> Result<()> {
    let batch_dir = server_data_dir.join(client_id).join(batch_id);

    if !batch_dir.exists() {
        anyhow::bail!("Batch directory does not exist: {:?}", batch_dir);
    }

    println!("  ✓ Batch directory exists: {:?}", batch_dir);

    // Check metadata.json exists
    let metadata_file = batch_dir.join("metadata.json");
    if !metadata_file.exists() {
        anyhow::bail!("Metadata file does not exist: {:?}", metadata_file);
    }

    println!("  ✓ Metadata file exists");

    // Validate metadata content
    let metadata_content =
        fs::read_to_string(&metadata_file).context("Failed to read metadata file")?;
    let metadata: serde_json::Value =
        serde_json::from_str(&metadata_content).context("Failed to parse metadata JSON")?;

    let filenames = metadata
        .get("filenames")
        .and_then(|v| v.as_array())
        .context("Invalid metadata format: missing filenames array")?;

    if filenames.len() != expected_file_count {
        anyhow::bail!(
            "Expected {} filenames in metadata, found {}",
            expected_file_count,
            filenames.len()
        );
    }

    println!(
        "  ✓ Metadata contains {} filenames (expected {})",
        filenames.len(),
        expected_file_count
    );

    // Validate all files exist
    for i in 0..expected_file_count {
        let filename = format!("file{}.txt", i);
        let file_path = batch_dir.join(&filename);

        if !file_path.exists() {
            anyhow::bail!("File does not exist: {:?}", file_path);
        }

        // Validate file content is not empty
        let content = fs::read(&file_path)
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;

        if content.is_empty() {
            anyhow::bail!("File is empty: {:?}", file_path);
        }

        // Verify filename is in metadata
        let filename_in_metadata = filenames.iter().any(|f| f.as_str() == Some(&filename));

        if !filename_in_metadata {
            anyhow::bail!("File {} not found in metadata", filename);
        }
    }

    println!(
        "  ✓ All {} files exist and have content",
        expected_file_count
    );
    println!("  ✓ All filenames are in metadata");

    // Validate public key exists
    let public_key_file = server_data_dir.join(client_id).join("public_key.hex");
    if public_key_file.exists() {
        let public_key =
            fs::read_to_string(&public_key_file).context("Failed to read public key file")?;
        if public_key.trim().is_empty() {
            anyhow::bail!("Public key file is empty");
        }
        println!("  ✓ Public key file exists and is not empty");
    } else {
        println!("  ⚠ Public key file does not exist (this is OK if using database storage)");
    }

    Ok(())
}

pub fn validate_client_data(
    client_data_dir: &Path,
    batch_id: &str,
    expected_file_count: usize,
) -> Result<()> {
    let batch_dir = client_data_dir.join(batch_id);

    if !batch_dir.exists() {
        anyhow::bail!("Client batch directory does not exist: {:?}", batch_dir);
    }

    println!("  ✓ Client batch directory exists: {:?}", batch_dir);

    // Validate root_hash.txt exists
    let root_hash_file = batch_dir.join("root_hash.txt");
    if !root_hash_file.exists() {
        anyhow::bail!("Root hash file does not exist: {:?}", root_hash_file);
    }

    let root_hash = fs::read_to_string(&root_hash_file).context("Failed to read root hash file")?;

    if root_hash.trim().is_empty() {
        anyhow::bail!("Root hash is empty");
    }

    if root_hash.trim().len() != 64 {
        anyhow::bail!(
            "Root hash has invalid length: {} (expected 64)",
            root_hash.trim().len()
        );
    }

    println!(
        "  ✓ Root hash exists and is valid (length: {})",
        root_hash.trim().len()
    );

    // Validate filenames.json exists
    let filenames_file = batch_dir.join("filenames.json");
    if !filenames_file.exists() {
        anyhow::bail!("Filenames file does not exist: {:?}", filenames_file);
    }

    let filenames_content =
        fs::read_to_string(&filenames_file).context("Failed to read filenames file")?;
    let filenames_json: serde_json::Value =
        serde_json::from_str(&filenames_content).context("Failed to parse filenames JSON")?;

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

    if filenames.len() != expected_file_count {
        anyhow::bail!(
            "Expected {} filenames, found {} in client data",
            expected_file_count,
            filenames.len()
        );
    }

    println!(
        "  ✓ Filenames.json contains {} filenames (expected {})",
        filenames.len(),
        expected_file_count
    );

    // Validate client_id.txt exists
    let client_id_file = client_data_dir.join("client_id.txt");
    if !client_id_file.exists() {
        anyhow::bail!("Client ID file does not exist: {:?}", client_id_file);
    }

    let client_id = fs::read_to_string(&client_id_file).context("Failed to read client ID file")?;

    if client_id.trim().is_empty() {
        anyhow::bail!("Client ID is empty");
    }

    println!("  ✓ Client ID file exists and is not empty");

    Ok(())
}

pub fn validate_downloaded_file(
    client_data_dir: &Path,
    batch_id: &str,
    filename: &str,
) -> Result<()> {
    let downloaded_dir = client_data_dir.join(batch_id).join("downloaded");
    let downloaded_file = downloaded_dir.join(filename);

    if !downloaded_file.exists() {
        anyhow::bail!("Downloaded file does not exist: {:?}", downloaded_file);
    }

    println!("  ✓ Downloaded file exists: {:?}", downloaded_file);

    // Validate file content is not empty
    let content = fs::read(&downloaded_file).context("Failed to read downloaded file")?;

    if content.is_empty() {
        anyhow::bail!("Downloaded file is empty");
    }

    println!("  ✓ Downloaded file has content ({} bytes)", content.len());

    // Validate file content matches expected pattern
    let content_str = String::from_utf8_lossy(&content);
    if !content_str.contains("Test file") {
        anyhow::bail!("Downloaded file content does not match expected pattern");
    }

    println!("  ✓ Downloaded file content matches expected pattern");

    Ok(())
}
