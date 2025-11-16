use anyhow::{Context, Result};
use sqlx::PgPool;

/// Query operations for database storage
pub struct Queries;

impl Queries {
    /// Ensure batch exists (create if not exists)
    pub async fn ensure_batch(
        pool: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
        client_id: &str,
        batch_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO batches (client_id, batch_id) VALUES ($1, $2) 
             ON CONFLICT (client_id, batch_id) DO NOTHING",
        )
        .bind(client_id)
        .bind(batch_id)
        .execute(pool)
        .await
        .context("Failed to ensure batch exists")?;
        Ok(())
    }

    /// Store file content
    pub async fn store_file(
        pool: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO files (client_id, batch_id, filename, content) VALUES ($1, $2, $3, $4)
             ON CONFLICT (client_id, batch_id, filename) DO UPDATE SET content = EXCLUDED.content",
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(filename)
        .bind(content)
        .execute(pool)
        .await
        .context("Failed to store file")?;
        Ok(())
    }

    /// Read file content
    pub async fn read_file(
        pool: &PgPool,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<Option<Vec<u8>>> {
        let row = sqlx::query_as::<_, (Vec<u8>,)>(
            "SELECT content FROM files WHERE client_id = $1 AND batch_id = $2 AND filename = $3",
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(filename)
        .fetch_optional(pool)
        .await
        .context("Failed to query file")?;

        Ok(row.map(|(content,)| content))
    }

    /// Check if batch exists
    pub async fn batch_exists(pool: &PgPool, client_id: &str, batch_id: &str) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM batches WHERE client_id = $1 AND batch_id = $2)",
        )
        .bind(client_id)
        .bind(batch_id)
        .fetch_one(pool)
        .await
        .context("Failed to check batch existence")?;
        Ok(exists)
    }

    /// Check if file exists
    pub async fn file_exists(
        pool: &PgPool,
        client_id: &str,
        batch_id: &str,
        filename: &str,
    ) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM files WHERE client_id = $1 AND batch_id = $2 AND filename = $3)",
        )
        .bind(client_id)
        .bind(batch_id)
        .bind(filename)
        .fetch_one(pool)
        .await
        .context("Failed to check file existence")?;
        Ok(exists)
    }

    /// Add filename to batch metadata
    pub async fn add_filename_to_metadata(
        pool: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
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
        .execute(pool)
        .await
        .context("Failed to add filename to metadata")?;
        Ok(())
    }

    /// Load batch filenames
    pub async fn load_batch_filenames(
        pool: &PgPool,
        client_id: &str,
        batch_id: &str,
    ) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT filename FROM batch_filenames 
             WHERE client_id = $1 AND batch_id = $2 ORDER BY filename",
        )
        .bind(client_id)
        .bind(batch_id)
        .fetch_all(pool)
        .await
        .context("Failed to load batch filenames")?;

        Ok(rows.into_iter().map(|(filename,)| filename).collect())
    }

    /// Read multiple files from a batch
    pub async fn read_batch_files(
        pool: &PgPool,
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
            .fetch_optional(pool)
            .await
            .with_context(|| format!("Failed to read file {}", filename))?;

            match row {
                Some((content,)) => file_data.push(content),
                None => anyhow::bail!("File {} not found in batch {}", filename, batch_id),
            }
        }

        Ok(file_data)
    }

    /// Store public key
    pub async fn store_public_key(pool: &PgPool, client_id: &str, public_key: &[u8]) -> Result<()> {
        sqlx::query(
            "INSERT INTO clients (client_id, public_key) VALUES ($1, $2)
             ON CONFLICT (client_id) DO UPDATE SET public_key = EXCLUDED.public_key",
        )
        .bind(client_id)
        .bind(public_key)
        .execute(pool)
        .await
        .context("Failed to store public key")?;
        Ok(())
    }

    /// Load public key
    pub async fn load_public_key(pool: &PgPool, client_id: &str) -> Result<Option<Vec<u8>>> {
        let row =
            sqlx::query_as::<_, (Vec<u8>,)>("SELECT public_key FROM clients WHERE client_id = $1")
                .bind(client_id)
                .fetch_optional(pool)
                .await
                .context("Failed to load public key")?;

        Ok(row.map(|(key,)| key))
    }
}
