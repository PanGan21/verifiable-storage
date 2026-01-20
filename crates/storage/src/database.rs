mod queries;
mod schema;
use merkle_tree::MerkleTree;

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

    async fn load_batch_filenames(&self, client_id: &str, batch_id: &str) -> Result<Vec<String>> {
        if !Queries::batch_exists(&self.pool, client_id, batch_id).await? {
            anyhow::bail!("Batch {} not found for client {}", batch_id, client_id);
        }
        Queries::load_batch_filenames(&self.pool, client_id, batch_id).await
    }

    async fn file_exists(&self, client_id: &str, batch_id: &str, filename: &str) -> Result<bool> {
        Queries::file_exists(&self.pool, client_id, batch_id, filename).await
    }

    async fn store_public_key(&self, client_id: &str, public_key: &[u8]) -> Result<()> {
        Queries::store_public_key(&self.pool, client_id, public_key).await
    }

    async fn load_public_key(&self, client_id: &str) -> Result<Option<Vec<u8>>> {
        Queries::load_public_key(&self.pool, client_id).await
    }

    async fn load_merkle_tree(
        &self,
        client_id: &str,
        batch_id: &str,
    ) -> Result<Option<merkle_tree::MerkleTree>> {
        Queries::load_merkle_tree(&self.pool, client_id, batch_id).await
    }

    async fn store_file_and_update_tree(
        &self,
        client_id: &str,
        batch_id: &str,
        filename: &str,
        content: &[u8],
        leaf_hash: &[u8; 32],
    ) -> Result<()> {
        // Use a single transaction to ensure atomicity
        // SELECT FOR UPDATE locks the leaf_hashes rows to prevent concurrent modifications
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction for atomic file and tree update")?;

        Queries::ensure_batch(&mut *tx, client_id, batch_id).await?;
        Queries::store_file(&mut *tx, client_id, batch_id, filename, content).await?;
        Queries::add_filename_to_metadata(&mut *tx, client_id, batch_id, filename).await?;
        Queries::store_leaf_hash(&mut *tx, client_id, batch_id, filename, leaf_hash).await?;

        // Load all leaf hashes with row-level lock
        // This prevents other transactions from modifying leaf hashes until committed
        let leaf_hashes = Queries::load_batch_leaf_hashes_locked(&mut *tx, client_id, batch_id)
            .await
            .context("Failed to load batch leaf hashes with lock")?;

        // Build Merkle tree from all leaf hashes
        let tree = MerkleTree::from_leaf_hashes(&leaf_hashes)
            .context("Failed to build Merkle tree from leaf hashes")?;

        // Store the rebuilt tree
        Queries::store_merkle_tree(&mut *tx, client_id, batch_id, &tree).await?;

        // Commit the entire transaction
        tx.commit()
            .await
            .context("Failed to commit transaction for atomic file and tree update")?;

        Ok(())
    }
}
