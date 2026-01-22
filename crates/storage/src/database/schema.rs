use anyhow::{Context, Result};
use sqlx::PgPool;
use tracing::info;

/// Database schema manager
pub struct Schema;

impl Schema {
    /// Initialize all database tables and indexes
    pub async fn initialize(pool: &PgPool) -> Result<()> {
        Self::create_clients_table(pool).await?;
        Self::create_batches_table(pool).await?;
        Self::create_files_table(pool).await?;
        Self::create_merkle_trees_table(pool).await?;
        Self::create_indexes(pool).await?;
        info!("PostgreSQL database storage initialized");
        Ok(())
    }

    /// Create clients table
    async fn create_clients_table(pool: &PgPool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS clients (
                client_id VARCHAR(255) PRIMARY KEY,
                public_key BYTEA NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await
        .context("Failed to create clients table")?;
        Ok(())
    }

    /// Create batches table
    async fn create_batches_table(pool: &PgPool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS batches (
                client_id VARCHAR(255) NOT NULL,
                batch_id VARCHAR(255) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (client_id, batch_id),
                FOREIGN KEY (client_id) REFERENCES clients(client_id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await
        .context("Failed to create batches table")?;
        Ok(())
    }

    /// Create files table
    async fn create_files_table(pool: &PgPool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS files (
                client_id VARCHAR(255) NOT NULL,
                batch_id VARCHAR(255) NOT NULL,
                filename VARCHAR(255) NOT NULL,
                content BYTEA NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (client_id, batch_id, filename),
                FOREIGN KEY (client_id, batch_id) REFERENCES batches(client_id, batch_id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await
        .context("Failed to create files table")?;
        Ok(())
    }

    /// Create merkle_trees table for storing Merkle tree structures
    async fn create_merkle_trees_table(pool: &PgPool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS merkle_trees (
                client_id VARCHAR(255) NOT NULL,
                batch_id VARCHAR(255) NOT NULL,
                tree_data JSONB NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (client_id, batch_id),
                FOREIGN KEY (client_id, batch_id) REFERENCES batches(client_id, batch_id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(pool)
        .await
        .context("Failed to create merkle_trees table")?;
        Ok(())
    }

    /// Create indexes for better query performance
    async fn create_indexes(pool: &PgPool) -> Result<()> {
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_batches_client ON batches(client_id)")
            .execute(pool)
            .await
            .context("Failed to create batches index")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_batch ON files(client_id, batch_id)")
            .execute(pool)
            .await
            .context("Failed to create files index")?;
        Ok(())
    }
}
