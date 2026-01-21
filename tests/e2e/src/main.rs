mod database_validator;
mod filesystem_validator;
mod test_utils;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use test_utils::*;

const TEST_FILES_COUNT: usize = 2;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("e2e_tests=debug,info")
        .init();

    let storage_type = std::env::var("STORAGE_TYPE").unwrap_or_else(|_| "database".to_string());

    match storage_type.as_str() {
        "database" => {
            println!("🗄️  Running E2E tests with DATABASE storage...");
            run_database_storage_tests().await?;
        }
        "filesystem" => {
            println!("📁 Running E2E tests with FILESYSTEM storage...");
            run_filesystem_storage_tests().await?;
        }
        _ => {
            anyhow::bail!(
                "Invalid STORAGE_TYPE: {}. Must be 'database' or 'filesystem'",
                storage_type
            );
        }
    }

    println!("\n✅ All E2E tests passed!");

    Ok(())
}

async fn run_database_storage_tests() -> Result<()> {
    let server_url =
        std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://verifiable_storage:verifiable_storage_password@localhost:5432/verifiable_storage".to_string());

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let test_dir = manifest_dir;
    let client_binary = workspace_root.join("target").join("release").join("client");

    // Create temporary test directories
    let test_data_dir = test_dir.join("test_data").join("database");
    let client_data_dir = test_data_dir.join("client_data");
    let test_files_dir = test_data_dir.join("test_files");

    std::fs::create_dir_all(&client_data_dir)?;
    std::fs::create_dir_all(&test_files_dir)?;

    println!("Server URL: {}", server_url);
    println!("Database URL: {}", database_url);
    println!("Client binary: {:?}", client_binary);

    // Wait for server to be ready
    wait_for_server(&server_url).await?;

    // Create test files
    create_test_files(&test_files_dir, TEST_FILES_COUNT)?;

    // Generate keypair
    println!("\n📝 Generating keypair...");
    let client_id = generate_keypair(&client_binary, &client_data_dir)?;
    println!("Generated client ID: {}", client_id);

    // Store client_id and batch_id for cleanup
    let batch_id = "e2e-test-batch-db-001".to_string();

    // Run tests
    let test_result = async {
        // Test upload
        println!("\n📤 Testing upload...");
        upload_files(
            &client_binary,
            &client_data_dir,
            &test_files_dir,
            &server_url,
            &batch_id,
        )?;

        // Validate database
        println!("\n🔍 Validating database state...");
        database_validator::validate_upload(&database_url, &client_id, &batch_id, TEST_FILES_COUNT)
            .await?;
        println!("✅ Database validation passed");

        // Validate client filesystem
        println!("\n🔍 Validating client filesystem...");
        filesystem_validator::validate_client_data(&client_data_dir, &batch_id, TEST_FILES_COUNT)?;
        println!("✅ Client filesystem validation passed");

        // Test download
        println!("\n📥 Testing download...");
        download_file(
            &client_binary,
            &client_data_dir,
            &server_url,
            &batch_id,
            "file0.txt",
        )?;

        // Validate downloaded file
        println!("\n🔍 Validating downloaded file...");
        filesystem_validator::validate_downloaded_file(&client_data_dir, &batch_id, "file0.txt")?;
        println!("✅ Downloaded file validation passed");

        // Test proof verification
        println!("\n🔍 Validating Merkle proof...");
        validate_merkle_proof(&client_data_dir, &batch_id, "file0.txt")?;
        println!("✅ Merkle proof validation passed");

        Ok::<(), anyhow::Error>(())
    };

    let result = test_result.await;

    // Always cleanup, even on error
    if let Err(e) = cleanup_test_data(&test_data_dir) {
        eprintln!("Warning: Failed to cleanup test data: {}", e);
    }
    if let Err(e) =
        database_validator::cleanup_test_data(&database_url, &client_id, &batch_id).await
    {
        eprintln!("Warning: Failed to cleanup database test data: {}", e);
    }

    result
}

async fn run_filesystem_storage_tests() -> Result<()> {
    let server_url =
        std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let test_dir = manifest_dir;
    let server_data_dir = std::env::var("SERVER_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            test_dir
                .join("test_data")
                .join("filesystem")
                .join("server_data")
        });
    let client_binary = workspace_root.join("target").join("release").join("client");

    // Create temporary test directories
    let test_data_dir = test_dir.join("test_data").join("filesystem");
    let client_data_dir = test_data_dir.join("client_data");
    let test_files_dir = test_data_dir.join("test_files");

    std::fs::create_dir_all(&client_data_dir)?;
    std::fs::create_dir_all(&server_data_dir)?;
    std::fs::create_dir_all(&test_files_dir)?;

    println!("Server URL: {}", server_url);
    println!("Server data dir: {:?}", server_data_dir);
    println!("Client binary: {:?}", client_binary);

    // Wait for server to be ready
    wait_for_server(&server_url).await?;

    // Create test files
    create_test_files(&test_files_dir, TEST_FILES_COUNT)?;

    // Generate keypair
    println!("\n📝 Generating keypair...");
    let client_id = generate_keypair(&client_binary, &client_data_dir)?;
    println!("Generated client ID: {}", client_id);

    // Store batch_id for cleanup
    let batch_id = "e2e-test-batch-fs-001".to_string();

    // Run tests
    let test_result = async {
        // Test upload
        println!("\n📤 Testing upload...");
        upload_files(
            &client_binary,
            &client_data_dir,
            &test_files_dir,
            &server_url,
            &batch_id,
        )?;

        // Validate server filesystem
        println!("\n🔍 Validating server filesystem...");
        filesystem_validator::validate_upload(
            &server_data_dir,
            &client_id,
            &batch_id,
            TEST_FILES_COUNT,
        )?;
        println!("✅ Server filesystem validation passed");

        // Validate client filesystem
        println!("\n🔍 Validating client filesystem...");
        filesystem_validator::validate_client_data(&client_data_dir, &batch_id, TEST_FILES_COUNT)?;
        println!("✅ Client filesystem validation passed");

        // Test download
        println!("\n📥 Testing download...");
        download_file(
            &client_binary,
            &client_data_dir,
            &server_url,
            &batch_id,
            "file0.txt",
        )?;

        // Validate downloaded file
        println!("\n🔍 Validating downloaded file...");
        filesystem_validator::validate_downloaded_file(&client_data_dir, &batch_id, "file0.txt")?;
        println!("✅ Downloaded file validation passed");

        // Test proof verification
        println!("\n🔍 Validating Merkle proof...");
        validate_merkle_proof(&client_data_dir, &batch_id, "file0.txt")?;
        println!("✅ Merkle proof validation passed");

        Ok::<(), anyhow::Error>(())
    };

    let result = test_result.await;

    // Always cleanup, even on error
    if let Err(e) = cleanup_test_data(&test_data_dir) {
        eprintln!("Warning: Failed to cleanup test data: {}", e);
    }
    if let Err(e) = cleanup_server_data(&server_data_dir) {
        eprintln!("Warning: Failed to cleanup server data: {}", e);
    }

    result
}

fn cleanup_test_data(test_data_dir: &Path) -> Result<()> {
    let keep_data = std::env::var("KEEP_TEST_DATA").unwrap_or_else(|_| "false".to_string());
    if keep_data == "true" {
        println!(
            "\n⚠️  Keeping test data (KEEP_TEST_DATA=true): {:?}",
            test_data_dir
        );
        return Ok(());
    }

    println!("\n🧹 Cleaning up test data: {:?}", test_data_dir);
    if test_data_dir.exists() {
        std::fs::remove_dir_all(test_data_dir).with_context(|| {
            format!("Failed to remove test data directory: {:?}", test_data_dir)
        })?;
        println!("✅ Test data cleaned up");
    }
    Ok(())
}

fn cleanup_server_data(server_data_dir: &Path) -> Result<()> {
    let keep_data = std::env::var("KEEP_TEST_DATA").unwrap_or_else(|_| "false".to_string());
    if keep_data == "true" {
        println!(
            "\n⚠️  Keeping server data (KEEP_TEST_DATA=true): {:?}",
            server_data_dir
        );
        return Ok(());
    }

    println!("\n🧹 Cleaning up server data: {:?}", server_data_dir);
    if server_data_dir.exists() {
        std::fs::remove_dir_all(server_data_dir).with_context(|| {
            format!(
                "Failed to remove server data directory: {:?}",
                server_data_dir
            )
        })?;
        println!("✅ Server data cleaned up");
    }
    Ok(())
}
