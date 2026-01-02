//! MCP server tool tests with mocked connector.
//!
//! These tests exercise the MCP tool handlers with a mocked `QdrantConnector`,
//! allowing us to test deprecation filtering and other server behaviour
//! without needing a real Qdrant instance.

use std::time::{SystemTime, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Content;
use rstest::rstest;

use super::tests::settings;
use super::*;
use crate::deprecation::SEVEN_DAYS_SECS;
use crate::tools::FindRequest;
use dear_diary_config::Settings;
use dear_diary_qdrant::{Entry, MockQdrantConnector, SearchResult};

/// Helper to get current Unix timestamp for tests.
fn current_timestamp() -> i64 {
    #[expect(clippy::cast_possible_wrap, reason = "Unix timestamp fits in i64")]
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .expect("System time should be after Unix epoch")
}

/// Helper to extract text from Content, panics if not text content.
fn content_text(content: &Content) -> &str {
    use rmcp::model::RawContent;
    match &content.raw {
        RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    }
}

/// Test that entries deprecated > 7 days ago are hidden in `qdrant_find` results.
#[rstest]
#[tokio::test]
async fn test_old_deprecated_entries_hidden_in_find(settings: Settings) {
    let mut connector = MockQdrantConnector::new();
    let now = current_timestamp();

    // Mock collection_exists to return true
    connector.expect_collection_exists().returning(|_| Ok(true));

    // Mock search to return an entry deprecated > 7 days ago
    let old_deprecated_ts = now - SEVEN_DAYS_SECS - 100;
    connector.expect_search().returning(move |_, _| {
        Ok(vec![SearchResult {
            point_id: "test-id".to_owned(),
            entry: Entry::new("Old deprecated memory"),
            deprecated_at: Some(old_deprecated_ts),
        }])
    });

    let server = DiaryServer::new(connector, settings);
    let request = FindRequest {
        query: "test".to_owned(),
        collection_name: None,
        filter: None,
        include_deprecated: false,
    };

    let result = server
        .qdrant_find(Parameters(request))
        .await
        .expect("qdrant_find should succeed");

    // The result should indicate no matches (entry is filtered out)
    let content = result.content.first().expect("Should have content");
    let text = content_text(content);
    assert!(
        text.contains("No results found"),
        "Old deprecated entry should be filtered out"
    );
}

/// Test that entries deprecated > 7 days ago are shown when `include_deprecated` is true.
#[rstest]
#[tokio::test]
async fn test_old_deprecated_entries_visible_with_flag(settings: Settings) {
    let mut connector = MockQdrantConnector::new();
    let now = current_timestamp();

    connector.expect_collection_exists().returning(|_| Ok(true));

    let old_deprecated_ts = now - SEVEN_DAYS_SECS - 100;
    connector.expect_search().returning(move |_, _| {
        Ok(vec![SearchResult {
            point_id: "test-id".to_owned(),
            entry: Entry::new("Old deprecated memory"),
            deprecated_at: Some(old_deprecated_ts),
        }])
    });

    let server = DiaryServer::new(connector, settings);
    let request = FindRequest {
        query: "test".to_owned(),
        collection_name: None,
        filter: None,
        include_deprecated: true,
    };

    let result = server
        .qdrant_find(Parameters(request))
        .await
        .expect("qdrant_find should succeed");

    // The result should contain the deprecated entry
    let content = result.content.first().expect("Should have content");
    let text = content_text(content);
    assert!(
        text.contains("[DEPRECATED]"),
        "Old deprecated entry should be visible with prefix"
    );
    assert!(
        text.contains("Old deprecated memory"),
        "Entry content should be present"
    );
}

/// Test that recently deprecated entries (< 7 days) are visible with [DEPRECATED] prefix.
#[rstest]
#[tokio::test]
async fn test_recent_deprecated_entries_visible_with_prefix(settings: Settings) {
    let mut connector = MockQdrantConnector::new();
    let now = current_timestamp();

    connector.expect_collection_exists().returning(|_| Ok(true));

    // Deprecated 1 day ago (less than 7 days)
    let recent_deprecated_ts = now - (24 * 3600);
    connector.expect_search().returning(move |_, _| {
        Ok(vec![SearchResult {
            point_id: "test-id".to_owned(),
            entry: Entry::new("Recent deprecated memory"),
            deprecated_at: Some(recent_deprecated_ts),
        }])
    });

    let server = DiaryServer::new(connector, settings);
    let request = FindRequest {
        query: "test".to_owned(),
        collection_name: None,
        filter: None,
        include_deprecated: false,
    };

    let result = server
        .qdrant_find(Parameters(request))
        .await
        .expect("qdrant_find should succeed");

    let content = result.content.first().expect("Should have content");
    let text = content_text(content);
    assert!(
        text.contains("[DEPRECATED]"),
        "Recently deprecated entry should show prefix"
    );
    assert!(
        text.contains("Recent deprecated memory"),
        "Entry content should be present"
    );
}
