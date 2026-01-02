//! Common test utilities for e2e tests.
//!
//! Provides shared fixtures and helpers for Qdrant integration testing.

#![allow(
    dead_code,
    reason = "Items shared across test binaries; not all used by each binary"
)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use dear_diary_config::DEFAULT_EMBEDDING_MODEL;
use dear_diary_embeddings::FastEmbedProvider;
use dear_diary_qdrant::QdrantConnectorImpl;
use dear_diary_qdrant::{QdrantConnector, SearchQuery};
use qdrant_client::Qdrant;

/// Check if Qdrant e2e tests should run based on environment variable.
pub fn should_run_qdrant_e2e() -> bool {
    matches!(std::env::var("E2E_QDRANT").as_deref(), Ok("1" | "true"))
}

/// Macro to skip e2e tests when `E2E_QDRANT` is not set.
#[macro_export]
macro_rules! skip_unless_e2e {
    () => {
        if !common::should_run_qdrant_e2e() {
            eprintln!("Skipping Qdrant e2e test; set E2E_QDRANT=1 to enable");
            return;
        }
    };
}

/// Test collection counter to ensure unique collection names per test.
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Test configuration loaded from environment.
pub struct TestConfig {
    pub qdrant_url: String,
    pub qdrant_api_key: String,
    pub collection_prefix: String,
    pub embedding_model: String,
}

impl TestConfig {
    /// Load test configuration from .env file.
    pub fn load() -> Self {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map_or_else(
                || std::path::PathBuf::from("."),
                std::path::Path::to_path_buf,
            );

        let env_path = workspace_root.join(".env");
        dotenvy::from_path(&env_path).ok();

        Self {
            qdrant_url: std::env::var("QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:6334".to_owned()),
            qdrant_api_key: std::env::var("QDRANT_API_KEY").unwrap_or_default(),
            collection_prefix: std::env::var("TEST_COLLECTION_PREFIX")
                .unwrap_or_else(|_| "dear-diary-test-".to_owned()),
            embedding_model: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| DEFAULT_EMBEDDING_MODEL.to_owned()),
        }
    }

    /// Generate a unique collection name for a test.
    pub fn unique_collection_name(&self, test_name: &str) -> String {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        format!(
            "{}{}-{}-{}",
            self.collection_prefix, test_name, timestamp, counter
        )
    }

    /// Create a Qdrant client for cleanup operations.
    pub fn create_cleanup_client(&self) -> Result<Qdrant, String> {
        let mut builder = Qdrant::from_url(&self.qdrant_url);
        if !self.qdrant_api_key.is_empty() {
            builder = builder.api_key(self.qdrant_api_key.clone());
        }

        builder
            .build()
            .map_err(|e| format!("Failed to connect to Qdrant: {e}"))
    }
}

/// Test fixture that creates a connector with a real Qdrant connection.
pub struct TestFixture {
    pub config: TestConfig,
    pub collection_name: String,
    pub connector: QdrantConnectorImpl<FastEmbedProvider>,
}

impl TestFixture {
    /// Create a new test fixture with a unique collection.
    pub fn new(test_name: &str) -> Result<Self, String> {
        let config = TestConfig::load();
        let collection_name = config.unique_collection_name(test_name);

        let embedding_provider = FastEmbedProvider::new(&config.embedding_model)
            .map_err(|e| format!("Failed to create embedding provider: {e}"))?;

        let connector = QdrantConnectorImpl::new(
            &config.qdrant_url,
            Some(config.qdrant_api_key.as_str()),
            Some(collection_name.clone()),
            embedding_provider,
        )
        .map_err(|e| format!("Failed to create Qdrant connector: {e}"))?;

        Ok(Self {
            config,
            collection_name,
            connector,
        })
    }

    /// Clean up the test collection.
    pub async fn cleanup(&self) -> Result<(), String> {
        let client = self.config.create_cleanup_client()?;

        if client
            .collection_exists(&self.collection_name)
            .await
            .unwrap_or(false)
        {
            client
                .delete_collection(&self.collection_name)
                .await
                .map_err(|e| {
                    format!("Failed to delete collection {}: {e}", self.collection_name)
                })?;
        }

        Ok(())
    }
}

/// Polls until at least one search result is found or timeout is reached.
///
/// This replaces fixed `tokio::time::sleep` calls after store operations,
/// allowing tests to proceed as soon as indexing completes rather than
/// waiting for a worst-case duration.
///
/// # Arguments
///
/// * `connector` - The Qdrant connector to use for searching
/// * `collection_name` - The collection to search in
/// * `query` - Search query text to find indexed entries
/// * `timeout` - Maximum time to wait for indexing
///
/// # Returns
///
/// `Ok(())` if results were found within timeout, `Err` otherwise.
pub async fn wait_for_indexing<C: QdrantConnector>(
    connector: &C,
    collection_name: &str,
    query: &str,
    timeout: Duration,
) -> Result<(), String> {
    const POLL_INTERVAL: Duration = Duration::from_millis(50);
    let start = std::time::Instant::now();

    loop {
        if connector
            .search(&SearchQuery::new(query, 1), collection_name)
            .await
            .is_ok_and(|results| !results.is_empty())
        {
            return Ok(());
        }

        if start.elapsed() >= timeout {
            return Err(format!("Timeout waiting for indexing after {timeout:?}"));
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
