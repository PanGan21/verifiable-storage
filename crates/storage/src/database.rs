//! PostgreSQL database storage implementation

use crate::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::info;

/// PostgreSQL database storage implementation
pub struct DatabaseStorage {
    pool: PgPool,
}

impl DatabaseStorage {
    /// Create a new PostgreSQL database storage instance
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .context("Failed to connect to PostgreSQL database")?;

        // Initialize database schema
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS clients (
                client_id VARCHAR(255) PRIMARY KEY,
                public_key BYTEA NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .context("Failed to create clients table")?;

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
        .execute(&pool)
        .await
        .context("Failed to create batches table")?;

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
        .execute(&pool)
        .await
        .context("Failed to create files table")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS batch_filenames (
                client_id VARCHAR(255) NOT NULL,
                batch_id VARCHAR(255) NOT NULL,
                filename VARCHAR(255) NOT NULL,
                PRIMARY KEY (client_id, batch_id, filename),
                FOREIGN KEY (client_id, batch_id, filename) 
                    REFERENCES files(client_id, batch_id, filename) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&pool)
        .await
        .context("Failed to create batch_filenames table")?;

        // Create indexes for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_batches_client ON batches(client_id)")
            .execute(&pool)
            .await
            .context("Failed to create batches index")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_batch ON files(client_id, batch_id)")
            .execute(&pool)
            .await
            .context("Failed to create files index")?;

        info!("PostgreSQL database storage initialized");
        Ok(Self { pool })
    }
}

#[async_trait]
impl Storage for DatabaseStorage {
    async fn store_file(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction")?;

        // Ensure batch exists (ignore if already exists)
        sqlx::query(
            "INSERT INTO batches (client_id, batch_id) VALUES ($1, $2) 
             ON CONFLICT (client_id, batch_id) DO NOTHING",
        )
        .bind(client_id)
        .bind(batch_id)
        .execute(&mut *tx)
        .await
        .context("Failed to create batch")?;

        // Store file
        sqlx::query(
            "INSERT INTO files (client_id, batch_id, filename, content) VALUES ($1, $2, $3, $4)
             ON CONFLICT (client_id, batch_id, filename) DO UPDATE SET content = EXCLUDED.content",
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(filename)
        .bind(content)
        .execute(&mut *tx)
        .await
        .context("Failed to store file")?;

        tx.commit().await.context("Failed to commit transaction")?;
        Ok(())
    }

    async fn read_file(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<Vec<u8>> {
        let row = sqlx::query_as::<_, (Vec<u8>,)>(
            "SELECT content FROM files WHERE client_id = $1 AND batch_id = $2 AND filename = $3",
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(filename)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query file")?;

        match row {
            Some((content,)) => Ok(content),
            None => anyhow::bail!(
                "File {} not found in batch {} for client {}",
                filename,
                batch_id,
                client_id
            ),
        }
    }

    async fn add_filename_to_metadata(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO batch_filenames (client_id, batch_id, filename) 
             VALUES ($1, $2, $3) ON CONFLICT (client_id, batch_id, filename) DO NOTHING",
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(filename)
        .execute(&self.pool)
        .await
        .context("Failed to add filename to metadata")?;

        Ok(())
    }

    async fn load_batch_filenames(&self, client_id: &str, batch_id: &str) -> Result<Vec<String>> {
        let batch_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM batches WHERE client_id = $1 AND batch_id = $2)",
        )
        .bind(client_id)
        .bind(batch_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check batch existence")?;

        if !batch_exists {
            anyhow::bail!("Batch {} not found for client {}", batch_id, client_id);
        }

        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT filename FROM batch_filenames 
             WHERE client_id = $1 AND batch_id = $2 ORDER BY filename",
        )
        .bind(client_id)
        .bind(batch_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to load batch filenames")?;

        Ok(rows.into_iter().map(|(filename,)| filename).collect())
    }

    async fn file_exists(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM files WHERE client_id = $1 AND batch_id = $2 AND filename = $3)",
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(filename)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check file existence")?;

        Ok(exists)
    }

    async fn read_batch_files(
        &self,
        client_id: &str,
        batch_id: &str,
        filenames: &[String],
    ) -> Result<Vec<Vec<u8>>> {
        let mut file_data = Vec::new();

        for filename in filenames {
            let row = sqlx::query_as::<_, (Vec<u8>,)>(
                "SELECT content FROM files WHERE client_id = $1 AND batch_id = $2 AND filename = $3",
            )
            .bind(client_id)
            .bind(batch_id)
            .bind(filename)
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("Failed to read file {}", filename))?;

            match row {
                Some((content,)) => file_data.push(content),
                None => anyhow::bail!("File {} not found in batch {}", filename, batch_id),
            }
        }

        Ok(file_data)
    }

    async fn store_public_key(&self, client_id: &str, public_key: &[u8]) -> Result<()> {
        sqlx::query(
            "INSERT INTO clients (client_id, public_key) VALUES ($1, $2)
             ON CONFLICT (client_id) DO UPDATE SET public_key = EXCLUDED.public_key",
        )
        .bind(client_id)
        .bind(public_key)
        .execute(&self.pool)
        .await
        .context("Failed to store public key")?;
        Ok(())
    }

    async fn load_public_key(&self, client_id: &str) -> Result<Option<Vec<u8>>> {
        let row =
            sqlx::query_as::<_, (Vec<u8>,)>("SELECT public_key FROM clients WHERE client_id = $1")
                .bind(client_id)
                .fetch_optional(&self.pool)
                .await
                .context("Failed to load public key")?;

        Ok(row.map(|(key,)| key))
    }

    async fn list_client_ids(&self) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT client_id FROM clients")
            .fetch_all(&self.pool)
            .await
            .context("Failed to list client IDs")?;

        Ok(rows.into_iter().map(|(client_id,)| client_id).collect())
    }
}
