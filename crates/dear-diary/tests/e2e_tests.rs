//! End-to-end tests for Dear Diary MCP server.
//!
//! These tests use a real Qdrant connection to verify the complete flow
//! of storing and retrieving information through the Qdrant connector.
//!
//! # Prerequisites
//!
//! - A `.env` file with Qdrant credentials (see `.env.example`)
//! - Network access to the Qdrant server
//! - `E2E_QDRANT=1` environment variable to enable tests
//!
//! # Running
//!
//! ```bash
//! E2E_QDRANT=1 cargo test -p dear-diary --test e2e_tests -- --test-threads=1
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};

/// Check if Qdrant e2e tests should run based on environment variable.
fn should_run_qdrant_e2e() -> bool {
    matches!(std::env::var("E2E_QDRANT").as_deref(), Ok("1" | "true"))
}

/// Macro to skip e2e tests when `E2E_QDRANT` is not set.
macro_rules! skip_unless_e2e {
    () => {
        if !should_run_qdrant_e2e() {
            eprintln!("Skipping Qdrant e2e test; set E2E_QDRANT=1 to enable");
            return;
        }
    };
}

use dear_diary_config::{DEFAULT_EMBEDDING_MODEL, QdrantSettings, Settings, ToolSettings};
use dear_diary_embeddings::FastEmbedProvider;
use dear_diary_mcp::DiaryServer;
use dear_diary_qdrant::{Entry, QdrantConnector, QdrantConnectorImpl, SearchQuery};
use insta::assert_json_snapshot;
use qdrant_client::Qdrant;
use rmcp::ServerHandler;
use rstest::rstest;
use serde_json::json;

/// Test collection counter to ensure unique collection names per test.
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Test configuration loaded from environment.
struct TestConfig {
    qdrant_url: String,
    qdrant_api_key: String,
    collection_prefix: String,
    embedding_model: String,
}

impl TestConfig {
    /// Load test configuration from .env file.
    fn load() -> Self {
        // Load .env file from workspace root
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
    fn unique_collection_name(&self, test_name: &str) -> String {
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
    fn create_cleanup_client(&self) -> Result<Qdrant, String> {
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
struct TestFixture {
    config: TestConfig,
    collection_name: String,
    connector: QdrantConnectorImpl<FastEmbedProvider>,
}

impl TestFixture {
    /// Create a new test fixture with a unique collection.
    fn new(test_name: &str) -> Result<Self, String> {
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
    async fn cleanup(&self) -> Result<(), String> {
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

// ============================================================================
// Store Feature Tests
// ============================================================================

/// Feature: Storing information in Qdrant
/// Scenario: Store and retrieve a simple memory
#[rstest]
#[tokio::test]
async fn test_store_and_retrieve_simple_memory() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("store_simple").expect("Failed to create test fixture");

    // Store information
    let entry = Entry::new("Remember to buy milk tomorrow");
    let store_result = fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await;

    assert!(
        store_result.is_ok(),
        "Store failed: {:?}",
        store_result.err()
    );

    // Wait briefly for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search for the stored information
    let search_result = fixture
        .connector
        .search(
            &SearchQuery::new("milk shopping", 10),
            &fixture.collection_name,
        )
        .await;

    assert!(
        search_result.is_ok(),
        "Search failed: {:?}",
        search_result.err()
    );

    let results = search_result.expect("Search should succeed");
    assert!(!results.is_empty(), "Search should return results");
    assert!(
        results
            .first()
            .is_some_and(|r| r.entry.content.contains("milk")),
        "Result should contain stored content"
    );

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Storing information in Qdrant
/// Scenario: Store information with metadata
#[rstest]
#[tokio::test]
async fn test_store_with_metadata() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("store_metadata").expect("Failed to create test fixture");

    // Store information with metadata
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("category".to_owned(), json!("work"));
    metadata.insert("priority".to_owned(), json!("high"));

    let entry = Entry::with_metadata("Important meeting with client at 3pm", metadata);
    let store_result = fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await;

    assert!(
        store_result.is_ok(),
        "Store with metadata failed: {:?}",
        store_result.err()
    );

    // Wait briefly for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search for the stored information
    let search_result = fixture
        .connector
        .search(
            &SearchQuery::new("meeting client", 10),
            &fixture.collection_name,
        )
        .await;

    assert!(
        search_result.is_ok(),
        "Search failed: {:?}",
        search_result.err()
    );

    let results = search_result.expect("Search should succeed");
    assert!(!results.is_empty(), "Search should return results");

    // Verify result contains metadata
    let first_result = results.first().expect("Should have at least one result");
    assert!(
        first_result.entry.metadata.is_some(),
        "Result should have metadata"
    );
    assert!(
        first_result.entry.content.contains("meeting"),
        "Result should contain stored content"
    );

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}

// ============================================================================
// Find Feature Tests
// ============================================================================

/// Feature: Finding information in Qdrant
/// Scenario: Search in empty/non-existent collection returns no results
#[rstest]
#[tokio::test]
async fn test_search_empty_collection() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("search_empty").expect("Failed to create test fixture");

    // Search in collection that doesn't exist yet
    let search_result = fixture
        .connector
        .search(&SearchQuery::new("anything", 10), &fixture.collection_name)
        .await;

    assert!(
        search_result.is_ok(),
        "Search should not fail for non-existent collection: {:?}",
        search_result.err()
    );

    let results = search_result.expect("Search should succeed");
    assert!(
        results.is_empty(),
        "Empty collection should return no results"
    );

    // Cleanup (nothing to clean, but call anyway for consistency)
    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Finding information in Qdrant
/// Scenario: Semantic search returns relevant results
#[rstest]
#[tokio::test]
async fn test_semantic_search() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("semantic_search").expect("Failed to create test fixture");

    // Store multiple related items
    let items = [
        "The cat sat on the warm windowsill",
        "My dog loves playing fetch in the park",
        "Birds were singing in the garden trees",
    ];

    for item in &items {
        let entry = Entry::new(*item);
        fixture
            .connector
            .store(&entry, &fixture.collection_name)
            .await
            .expect("Store should succeed");
    }

    // Wait for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // Semantic search for "pet animals"
    let search_result = fixture
        .connector
        .search(
            &SearchQuery::new("pet animals", 10),
            &fixture.collection_name,
        )
        .await;

    assert!(
        search_result.is_ok(),
        "Search failed: {:?}",
        search_result.err()
    );

    let results = search_result.expect("Search should succeed");
    assert!(!results.is_empty(), "Should return results");

    // Cat and dog should be semantically related to "pet animals"
    let first_content = &results.first().expect("Should have results").entry.content;
    let has_pets = first_content.contains("cat") || first_content.contains("dog");
    assert!(has_pets, "Semantic search should find pet-related entries");

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Finding information in Qdrant
/// Scenario: Search limit is respected
#[rstest]
#[tokio::test]
async fn test_search_limit() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("search_limit").expect("Failed to create test fixture");

    // Store multiple items
    for i in 0..5 {
        let entry = Entry::new(format!("Test memory number {i}"));
        fixture
            .connector
            .store(&entry, &fixture.collection_name)
            .await
            .expect("Store should succeed");
    }

    // Wait for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // Search with limit of 2
    let search_result = fixture
        .connector
        .search(&SearchQuery::new("memory", 2), &fixture.collection_name)
        .await;

    assert!(
        search_result.is_ok(),
        "Search failed: {:?}",
        search_result.err()
    );

    let results = search_result.expect("Search should succeed");
    assert!(
        results.len() <= 2,
        "Should respect limit, got {} results",
        results.len()
    );

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}

// ============================================================================
// Server Info Snapshot Tests
// ============================================================================

/// Feature: Server information
/// Scenario: Server returns correct capabilities
#[rstest]
fn test_server_info_snapshot() {
    let config = TestConfig::load();
    let collection_name = config.unique_collection_name("server_info");

    let settings = Settings {
        tools: ToolSettings::default(),
        qdrant: QdrantSettings {
            qdrant_url: Some(config.qdrant_url.clone()),
            qdrant_api_key: Some(config.qdrant_api_key.clone()),
            collection_name: Some(collection_name),
            qdrant_local_path: None,
            search_limit: 10,
            read_only: false,
            filterable_fields: Vec::new(),
            allow_arbitrary_filter: false,
        },
        embedding_model: config.embedding_model.clone(),
    };

    let embedding_provider =
        FastEmbedProvider::new(&settings.embedding_model).expect("Failed to create provider");

    let connector = QdrantConnectorImpl::new(
        &config.qdrant_url,
        Some(config.qdrant_api_key.as_str()),
        settings.qdrant.collection_name.clone(),
        embedding_provider,
    )
    .expect("Failed to create connector");

    let server = DiaryServer::new(connector, settings);
    let info = server.get_info();

    // Create a serializable representation for snapshot
    let snapshot_data = json!({
        "name": info.server_info.name,
        "has_tools": info.capabilities.tools.is_some(),
        "has_instructions": info.instructions.is_some(),
    });

    assert_json_snapshot!("server_info", snapshot_data);
}

/// Feature: Collection existence check
/// Scenario: Collection exists after storing data
#[rstest]
#[tokio::test]
async fn test_collection_exists_after_store() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("collection_exists").expect("Failed to create test fixture");

    // Initially collection doesn't exist
    let exists_before = fixture
        .connector
        .collection_exists(&fixture.collection_name)
        .await
        .expect("Collection check should succeed");

    assert!(!exists_before, "Collection should not exist initially");

    // Store something to create the collection
    let entry = Entry::new("Test entry");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    // Now collection should exist
    let exists_after = fixture
        .connector
        .collection_exists(&fixture.collection_name)
        .await
        .expect("Collection check should succeed");

    assert!(exists_after, "Collection should exist after storing");

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}

// ============================================================================
// Read-Only Mode Tests
// ============================================================================

/// Feature: Read-only mode
/// Scenario: Read-only settings are properly configured
#[rstest]
fn test_read_only_mode_settings() {
    let config = TestConfig::load();
    let collection_name = config.unique_collection_name("read_only");

    let settings = Settings {
        tools: ToolSettings::default(),
        qdrant: QdrantSettings {
            qdrant_url: Some(config.qdrant_url.clone()),
            qdrant_api_key: Some(config.qdrant_api_key.clone()),
            collection_name: Some(collection_name),
            qdrant_local_path: None,
            search_limit: 10,
            read_only: true, // Enable read-only mode
            filterable_fields: Vec::new(),
            allow_arbitrary_filter: false,
        },
        embedding_model: config.embedding_model.clone(),
    };

    // Verify read-only is set correctly
    assert!(
        settings.qdrant.read_only,
        "Read-only mode should be enabled"
    );

    let embedding_provider =
        FastEmbedProvider::new(&settings.embedding_model).expect("Failed to create provider");

    let connector = QdrantConnectorImpl::new(
        &config.qdrant_url,
        Some(config.qdrant_api_key.as_str()),
        settings.qdrant.collection_name.clone(),
        embedding_provider,
    )
    .expect("Failed to create connector");

    let server = DiaryServer::new(connector, settings);
    let info = server.get_info();

    // Server should still have tools capability (enforcement is at runtime)
    assert!(info.capabilities.tools.is_some());
}

// ============================================================================
// Deprecation Feature Tests
// ============================================================================

/// Feature: Deprecating memories
/// Scenario: Deprecate an entry and verify it's marked as deprecated
#[rstest]
#[tokio::test]
async fn test_deprecate_entry() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("deprecate_entry").expect("Failed to create test fixture");

    // Store an entry
    let entry = Entry::new("Remember to call the dentist");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    // Wait for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search and get the point ID
    let results = fixture
        .connector
        .search(&SearchQuery::new("dentist", 1), &fixture.collection_name)
        .await
        .expect("Search should succeed");

    assert!(!results.is_empty(), "Should find the entry");
    let result = results.first().expect("Should have at least one result");
    assert!(
        result.deprecated_at.is_none(),
        "Entry should not be deprecated initially"
    );

    // Deprecate the entry
    fixture
        .connector
        .deprecate(&result.point_id, &fixture.collection_name)
        .await
        .expect("Deprecate should succeed");

    // Wait for update to propagate
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search again and verify deprecated_at is set
    let results_after = fixture
        .connector
        .search(&SearchQuery::new("dentist", 1), &fixture.collection_name)
        .await
        .expect("Search should succeed");

    assert!(!results_after.is_empty(), "Should still find the entry");
    let result_after = results_after.first().expect("Should have result");
    assert!(
        result_after.deprecated_at.is_some(),
        "Entry should be marked as deprecated"
    );

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Deprecating memories
/// Scenario: Recently deprecated entries are still visible but flagged
#[rstest]
#[tokio::test]
async fn test_deprecated_entry_visible_within_7_days() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("deprecated_visible").expect("Failed to create test fixture");

    // Store an entry
    let entry = Entry::new("Pick up dry cleaning");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    // Wait for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Get the entry and deprecate it
    let results = fixture
        .connector
        .search(
            &SearchQuery::new("dry cleaning", 1),
            &fixture.collection_name,
        )
        .await
        .expect("Search should succeed");

    let result = results.first().expect("Should find entry");
    fixture
        .connector
        .deprecate(&result.point_id, &fixture.collection_name)
        .await
        .expect("Deprecate should succeed");

    // Wait for update
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search again - entry should still be visible (deprecated < 7 days)
    let results_after = fixture
        .connector
        .search(
            &SearchQuery::new("dry cleaning", 1),
            &fixture.collection_name,
        )
        .await
        .expect("Search should succeed");

    assert!(
        !results_after.is_empty(),
        "Recently deprecated entry should still be visible"
    );
    assert!(
        results_after
            .first()
            .is_some_and(|r| r.deprecated_at.is_some()),
        "Entry should be marked as deprecated"
    );

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Deprecating memories
/// Scenario: `SearchResult` includes `point_id` for subsequent operations
#[rstest]
#[tokio::test]
async fn test_search_result_includes_point_id() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("point_id").expect("Failed to create test fixture");

    // Store an entry
    let entry = Entry::new("Test entry for point ID verification");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    // Wait for indexing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Search and verify point_id is present
    let results = fixture
        .connector
        .search(&SearchQuery::new("point ID", 1), &fixture.collection_name)
        .await
        .expect("Search should succeed");

    assert!(!results.is_empty(), "Should find the entry");
    let result = results.first().expect("Should have result");
    assert!(
        !result.point_id.is_empty(),
        "SearchResult should include a non-empty point_id"
    );

    // Cleanup
    fixture.cleanup().await.expect("Cleanup failed");
}
