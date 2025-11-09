mod queries;
mod schema;

use crate::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use queries::Queries;
use schema::Schema;
use sqlx::PgPool;

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
        Schema::initialize(&pool).await?;

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

        Queries::ensure_batch(&mut *tx, client_id, batch_id).await?;
        Queries::store_file(&mut *tx, client_id, batch_id, filename, content).await?;

        tx.commit().await.context("Failed to commit transaction")?;
        Ok(())
    }

    async fn read_file(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<Vec<u8>> {
        Queries::read_file(&self.pool, client_id, batch_id, filename)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "File {} not found in batch {} for client {}",
                    filename,
                    batch_id,
                    client_id
                )
            })
    }

    async fn add_filename_to_metadata(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<()> {
        Queries::add_filename_to_metadata(&self.pool, client_id, batch_id, filename).await
    }

    async fn load_batch_filenames(&self, client_id: &str, batch_id: &str) -> Result<Vec<String>> {
        if !Queries::batch_exists(&self.pool, client_id, batch_id).await? {
            anyhow::bail!("Batch {} not found for client {}", batch_id, client_id);
        }
        Queries::load_batch_filenames(&self.pool, client_id, batch_id).await
    }

    async fn file_exists(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<bool> {
        Queries::file_exists(&self.pool, client_id, batch_id, filename).await
    }

    async fn read_batch_files(
        &self,
        client_id: &str,
        batch_id: &str,
        filenames: &[String],
    ) -> Result<Vec<Vec<u8>>> {
        Queries::read_batch_files(&self.pool, client_id, batch_id, filenames).await
    }

    async fn store_public_key(&self, client_id: &str, public_key: &[u8]) -> Result<()> {
        Queries::store_public_key(&self.pool, client_id, public_key).await
    }

    async fn load_public_key(&self, client_id: &str) -> Result<Option<Vec<u8>>> {
        Queries::load_public_key(&self.pool, client_id).await
    }

    async fn list_client_ids(&self) -> Result<Vec<String>> {
        Queries::list_client_ids(&self.pool).await
    }
}
