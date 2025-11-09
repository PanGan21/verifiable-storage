mod queries;
mod schema;

use crate::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use queries::Queries;
use schema::Schema;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

/// Database connection retry configuration
#[derive(Debug, Clone)]
pub struct DatabaseRetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay in seconds for exponential backoff
    pub initial_delay_seconds: u64,
}

impl Default for DatabaseRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay_seconds: 1,
        }
    }
}

impl DatabaseRetryConfig {
    /// Create retry config from environment variables
    /// Reads DB_RETRY_MAX_ATTEMPTS and DB_RETRY_INITIAL_DELAY_SECONDS
    pub fn from_env() -> Self {
        let max_attempts = std::env::var("DB_RETRY_MAX_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let initial_delay_seconds = std::env::var("DB_RETRY_INITIAL_DELAY_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        Self {
            max_attempts,
            initial_delay_seconds,
        }
    }
}

/// PostgreSQL database storage implementation
pub struct DatabaseStorage {
    pool: PgPool,
}

impl DatabaseStorage {
    /// Create a new PostgreSQL database storage instance
    /// Uses default retry configuration
    pub async fn new(database_url: &str) -> Result<Self> {
        Self::new_with_retry_config(database_url, DatabaseRetryConfig::default()).await
    }

    /// Create a new PostgreSQL database storage instance with custom retry configuration
    pub async fn new_with_retry_config(
        database_url: &str,
        retry_config: DatabaseRetryConfig,
    ) -> Result<Self> {
        let pool = connect_with_retry(database_url, &retry_config).await?;
        Schema::initialize(&pool).await?;
        Ok(Self { pool })
    }
}

/// Connect to PostgreSQL database with retry logic and exponential backoff
async fn connect_with_retry(database_url: &str, config: &DatabaseRetryConfig) -> Result<PgPool> {
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match PgPool::connect(database_url).await {
            Ok(pool) => {
                // Test the connection
                if let Err(e) = sqlx::query("SELECT 1").execute(&pool).await {
                    last_error = Some(format!("Failed to test database connection: {}", e));
                    if attempt < config.max_attempts - 1 {
                        let delay = calculate_retry_delay(attempt, config.initial_delay_seconds);
                        warn!(
                            "Database connection test failed, retrying in {:?}... (attempt {}/{})",
                            delay,
                            attempt + 1,
                            config.max_attempts
                        );
                        sleep(delay).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!("{}", last_error.unwrap()));
                }

                // Connection successful
                return Ok(pool);
            }
            Err(e) => {
                last_error = Some(format!("Failed to connect to PostgreSQL database: {}", e));
                if attempt < config.max_attempts - 1 {
                    let delay = calculate_retry_delay(attempt, config.initial_delay_seconds);
                    warn!(
                        "Database connection failed, retrying in {:?}... (attempt {}/{})",
                        delay,
                        attempt + 1,
                        config.max_attempts
                    );
                    sleep(delay).await;
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to connect to database after {} attempts: {}",
        config.max_attempts,
        last_error.unwrap_or_else(|| "Unknown error".to_string())
    ))
}

/// Calculate retry delay using exponential backoff
fn calculate_retry_delay(attempt: u32, initial_delay_seconds: u64) -> Duration {
    Duration::from_secs(initial_delay_seconds * 2_u64.pow(attempt))
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
