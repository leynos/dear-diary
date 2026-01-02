//! End-to-end tests for server info and configuration.
//!
//! Tests server capabilities, info snapshots, and read-only mode.

mod common;

use dear_diary_config::{QdrantSettings, Settings, ToolSettings};
use dear_diary_embeddings::FastEmbedProvider;
use dear_diary_mcp::DiaryServer;
use dear_diary_qdrant::QdrantConnectorImpl;
use insta::assert_json_snapshot;
use rmcp::ServerHandler;
use rstest::{fixture, rstest};
use serde_json::json;

use crate::common::TestConfig;

/// Shared server configuration for e2e tests.
struct ServerTestContext {
    server: DiaryServer<QdrantConnectorImpl<FastEmbedProvider>>,
    settings: Settings,
}

/// Creates a server with the given test name and `read_only` setting.
#[expect(
    clippy::expect_used,
    reason = "Test fixture setup may panic on failure"
)]
fn create_server_context(test_name: &str, read_only: bool) -> ServerTestContext {
    let config = TestConfig::load();
    let collection_name = config.unique_collection_name(test_name);

    let settings = Settings {
        tools: ToolSettings::default(),
        qdrant: QdrantSettings {
            qdrant_url: Some(config.qdrant_url.clone()),
            qdrant_api_key: Some(config.qdrant_api_key.clone()),
            collection_name: Some(collection_name),
            qdrant_local_path: None,
            search_limit: 10,
            read_only,
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

    let server = DiaryServer::new(connector, settings.clone());
    ServerTestContext { server, settings }
}

#[fixture]
fn server_info_context() -> ServerTestContext {
    create_server_context("server_info", false)
}

#[fixture]
fn read_only_context() -> ServerTestContext {
    create_server_context("read_only", true)
}

/// Feature: Server information
/// Scenario: Server returns correct capabilities
#[rstest]
fn test_server_info_snapshot(server_info_context: ServerTestContext) {
    let info = server_info_context.server.get_info();

    let snapshot_data = json!({
        "name": info.server_info.name,
        "has_tools": info.capabilities.tools.is_some(),
        "has_instructions": info.instructions.is_some(),
    });

    assert_json_snapshot!("server_info", snapshot_data);
}

/// Feature: Read-only mode
/// Scenario: Read-only settings are properly configured
#[rstest]
fn test_read_only_mode_settings(read_only_context: ServerTestContext) {
    assert!(
        read_only_context.settings.qdrant.read_only,
        "Read-only mode should be enabled"
    );

    let info = read_only_context.server.get_info();
    assert!(
        info.capabilities.tools.is_some(),
        "Server should have tools enabled even in read-only mode"
    );
}
