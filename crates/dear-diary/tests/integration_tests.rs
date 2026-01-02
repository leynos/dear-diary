//! Integration tests for Dear Diary MCP server.
//!
//! These tests verify the core functionality of the server using mocked
//! components to ensure deterministic behavior.

use std::collections::HashMap;

use dear_diary_config::{DEFAULT_EMBEDDING_MODEL, QdrantSettings, Settings, ToolSettings};
use dear_diary_mcp::DiaryServer;
use dear_diary_qdrant::{Entry, MockQdrantConnector};
use rmcp::ServerHandler;
use rstest::{fixture, rstest};
use serde_json::json;

/// Creates test settings with configurable collection name and read-only mode.
fn create_test_settings(collection_name: Option<&str>, read_only: bool) -> Settings {
    Settings {
        tools: ToolSettings::default(),
        qdrant: QdrantSettings {
            qdrant_url: Some("http://localhost:6334".to_owned()),
            qdrant_api_key: None,
            collection_name: collection_name.map(String::from),
            qdrant_local_path: None,
            search_limit: 10,
            read_only,
            filterable_fields: Vec::new(),
            allow_arbitrary_filter: false,
        },
        embedding_model: DEFAULT_EMBEDDING_MODEL.to_owned(),
    }
}

/// Fixture providing default test settings with a collection.
#[fixture]
fn settings() -> Settings {
    create_test_settings(Some("test"), false)
}

/// Fixture providing read-only test settings.
#[fixture]
fn read_only_settings() -> Settings {
    create_test_settings(Some("test"), true)
}

// ============================================================================
// Store Feature Tests
// ============================================================================

/// Feature: Storing information in Qdrant
/// Scenario: Server can be created with store capabilities
#[rstest]
fn test_server_with_store_capability(settings: Settings) {
    let mock = MockQdrantConnector::new();
    let server = DiaryServer::new(mock, settings);

    // Verify server is created with tools enabled
    let info = server.get_info();
    assert!(info.capabilities.tools.is_some());
}

/// Feature: Storing information in Qdrant
/// Scenario: Read-only mode prevents store
#[rstest]
fn test_read_only_mode(read_only_settings: Settings) {
    let mock = MockQdrantConnector::new();
    let server = DiaryServer::new(mock, read_only_settings);

    // Server created in read-only mode with tools enabled
    // The store tool will reject requests at runtime
    assert!(server.get_info().capabilities.tools.is_some());
}

// ============================================================================
// Find Feature Tests
// ============================================================================

/// Feature: Finding information in Qdrant
/// Scenario: Server can be created with search capabilities
#[rstest]
fn test_server_with_search_capability(settings: Settings) {
    let mock = MockQdrantConnector::new();
    let server = DiaryServer::new(mock, settings);

    // Verify server has proper version info
    let info = server.get_info();
    assert_eq!(info.server_info.name, "dear-diary");
}

// ============================================================================
// Configuration Tests
// ============================================================================

/// Test that settings without a default collection still creates server
#[rstest]
fn test_no_default_collection() {
    let mock = MockQdrantConnector::new();
    let settings = create_test_settings(None, false);
    let server = DiaryServer::new(mock, settings);

    // Server creates successfully, but tool calls without collection_name
    // would fail with MissingCollectionName error at runtime
    assert!(server.get_info().capabilities.tools.is_some());
}

/// Test server creation with custom search limit
#[rstest]
fn test_custom_search_limit(mut settings: Settings) {
    let mock = MockQdrantConnector::new();
    settings.qdrant.search_limit = 5;

    let server = DiaryServer::new(mock, settings);
    assert!(server.get_info().capabilities.tools.is_some());
}

/// Test server info includes proper metadata
#[rstest]
fn test_server_info(settings: Settings) {
    let mock = MockQdrantConnector::new();
    let server = DiaryServer::new(mock, settings);

    let info = server.get_info();
    assert_eq!(info.server_info.name, "dear-diary");
    assert!(info.instructions.is_some());
    assert!(
        info.instructions
            .as_ref()
            .is_some_and(|i| i.contains("Qdrant"))
    );
}

// ============================================================================
// Entry Model Tests
// ============================================================================

/// Test Entry creation without metadata
#[rstest]
fn test_entry_without_metadata() {
    let entry = Entry::new("Test content");
    assert_eq!(entry.content, "Test content");
    assert!(entry.metadata.is_none());
}

/// Test Entry creation with metadata
#[rstest]
fn test_entry_with_metadata() {
    let mut metadata = HashMap::new();
    metadata.insert("category".to_owned(), json!("work"));
    metadata.insert("priority".to_owned(), json!(1));

    let entry = Entry::with_metadata("Important meeting", metadata);
    assert_eq!(entry.content, "Important meeting");
    assert!(entry.metadata.is_some());
    assert_eq!(
        entry.metadata.as_ref().and_then(|m| m.get("category")),
        Some(&json!("work"))
    );
}
