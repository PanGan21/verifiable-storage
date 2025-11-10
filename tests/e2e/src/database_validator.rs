use anyhow::{Context, Result};
use sqlx::PgPool;

pub async fn validate_upload(
    database_url: &str,
    client_id: &str,
    batch_id: &str,
    expected_file_count: usize,
) -> Result<()> {
    let pool = PgPool::connect(database_url)
        .await
        .context("Failed to connect to database")?;

    // Validate client exists
    let client_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM clients WHERE client_id = $1)")
            .bind(client_id)
            .fetch_one(&pool)
            .await
            .context("Failed to check if client exists")?;

    if !client_exists {
        anyhow::bail!("Client {} not found in database", client_id);
    }

    println!("  ✓ Client {} exists in database", client_id);

    // Validate batch exists
    let batch_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM batches WHERE client_id = $1 AND batch_id = $2)",
    )
    .bind(client_id)
    .bind(batch_id)
    .fetch_one(&pool)
    .await
    .context("Failed to check if batch exists")?;

    if !batch_exists {
        anyhow::bail!("Batch {} not found for client {}", batch_id, client_id);
    }

    println!("  ✓ Batch {} exists for client {}", batch_id, client_id);

    // Validate files count
    let file_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE client_id = $1 AND batch_id = $2")
            .bind(client_id)
            .bind(batch_id)
            .fetch_one(&pool)
            .await
            .context("Failed to count files")?;

    if file_count != expected_file_count as i64 {
        anyhow::bail!(
            "Expected {} files, found {} in database",
            expected_file_count,
            file_count
        );
    }

    println!(
        "  ✓ Found {} files in batch (expected {})",
        file_count, expected_file_count
    );

    // Validate batch_filenames count
    let metadata_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM batch_filenames WHERE client_id = $1 AND batch_id = $2",
    )
    .bind(client_id)
    .bind(batch_id)
    .fetch_one(&pool)
    .await
    .context("Failed to count batch_filenames")?;

    if metadata_count != expected_file_count as i64 {
        anyhow::bail!(
            "Expected {} filenames in metadata, found {}",
            expected_file_count,
            metadata_count
        );
    }

    println!(
        "  ✓ Found {} filenames in metadata (expected {})",
        metadata_count, expected_file_count
    );

    // Validate that all files have content
    let files_with_content: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM files WHERE client_id = $1 AND batch_id = $2 AND LENGTH(content) > 0",
    )
    .bind(client_id)
    .bind(batch_id)
    .fetch_one(&pool)
    .await
    .context("Failed to check file content")?;

    if files_with_content != expected_file_count as i64 {
        anyhow::bail!(
            "Expected {} files with content, found {}",
            expected_file_count,
            files_with_content
        );
    }

    println!("  ✓ All {} files have content", files_with_content);

    // Validate specific filenames exist
    for i in 0..expected_file_count {
        let filename = format!("file{}.txt", i);
        let file_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM files WHERE client_id = $1 AND batch_id = $2 AND filename = $3)"
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(&filename)
        .fetch_one(&pool)
        .await
        .with_context(|| format!("Failed to check if file {} exists", filename))?;

        if !file_exists {
            anyhow::bail!("File {} not found in database", filename);
        }
    }

    println!("  ✓ All expected filenames exist in database");

    Ok(())
}

/// Clean up test data from database
pub async fn cleanup_test_data(database_url: &str, client_id: &str, batch_id: &str) -> Result<()> {
    let keep_data = std::env::var("KEEP_TEST_DATA").unwrap_or_else(|_| "false".to_string());
    if keep_data == "true" {
        println!("⚠️  Keeping database test data (KEEP_TEST_DATA=true)");
        return Ok(());
    }

    let pool = sqlx::PgPool::connect(database_url)
        .await
        .context("Failed to connect to database for cleanup")?;

    println!("🧹 Cleaning up database test data...");

    // Delete batch (cascade will delete files and batch_filenames)
    sqlx::query("DELETE FROM batches WHERE client_id = $1 AND batch_id = $2")
        .bind(client_id)
        .bind(batch_id)
        .execute(&pool)
        .await
        .context("Failed to delete batch from database")?;

    // Delete client (only if no other batches exist)
    let batch_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM batches WHERE client_id = $1")
        .bind(client_id)
        .fetch_one(&pool)
        .await
        .context("Failed to check batch count")?;

    if batch_count == 0 {
        sqlx::query("DELETE FROM clients WHERE client_id = $1")
            .bind(client_id)
            .execute(&pool)
            .await
            .context("Failed to delete client from database")?;
        println!("✅ Client and all associated data cleaned up");
    } else {
        println!("✅ Batch cleaned up (client still has other batches)");
    }

    Ok(())
}
