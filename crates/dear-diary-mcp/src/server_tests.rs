//! MCP server tool tests with mocked connector.
//!
//! These tests exercise the MCP tool handlers with a mocked `QdrantConnector`,
//! allowing us to test deprecation filtering and other server behaviour
//! without needing a real Qdrant instance.

use std::time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Content;
use rstest::rstest;

use super::tests::settings;
use super::*;
use crate::deprecation::SEVEN_DAYS_SECS;
use crate::tools::{DeprecateRequest, FindRequest, StoreRequest};
use dear_diary_config::Settings;
use dear_diary_qdrant::{Entry, MockQdrantConnector, QdrantError, SearchResult};
use rmcp::model::ErrorCode;

/// One day in seconds (24 hours × 3600 seconds).
const ONE_DAY_SECS: i64 = 24 * 3600;

/// Helper to get current Unix timestamp for tests.
fn current_timestamp() -> Result<i64, SystemTimeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(duration_secs_i64)
}

/// Converts a Unix timestamp duration into signed seconds for test fixtures.
#[expect(clippy::cast_possible_wrap, reason = "Unix timestamp fits in i64")]
fn duration_secs_i64(duration: Duration) -> i64 {
    duration.as_secs() as i64
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
async fn test_old_deprecated_entries_hidden_in_find(
    settings: Settings,
) -> Result<(), SystemTimeError> {
    let mut connector = MockQdrantConnector::new();
    let now = current_timestamp()?;

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
    Ok(())
}

/// Test that entries deprecated > 7 days ago are shown when `include_deprecated` is true.
#[rstest]
#[tokio::test]
async fn test_old_deprecated_entries_visible_with_flag(
    settings: Settings,
) -> Result<(), SystemTimeError> {
    let mut connector = MockQdrantConnector::new();
    let now = current_timestamp()?;

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
    Ok(())
}

/// Test that recently deprecated entries (< 7 days) are visible with [DEPRECATED] prefix.
#[rstest]
#[tokio::test]
async fn test_recent_deprecated_entries_visible_with_prefix(
    settings: Settings,
) -> Result<(), SystemTimeError> {
    let mut connector = MockQdrantConnector::new();
    let now = current_timestamp()?;

    connector.expect_collection_exists().returning(|_| Ok(true));

    // Deprecated 1 day ago (less than 7 days)
    let recent_deprecated_ts = now - ONE_DAY_SECS;
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
    Ok(())
}

/// Test that `qdrant_store` rejects requests when server is in read-only mode.
#[rstest]
#[tokio::test]
async fn test_read_only_mode_rejects_store(mut settings: Settings) {
    settings.qdrant.read_only = true;
    let connector = MockQdrantConnector::new();

    let server = DiaryServer::new(connector, settings);
    let request = crate::tools::StoreRequest {
        information: "Test content".to_owned(),
        metadata: None,
        collection_name: None,
    };

    let result = server.qdrant_store(Parameters(request)).await;

    let err = result.expect_err("Store should fail in read-only mode");
    assert_eq!(
        err.code,
        ErrorCode::INVALID_REQUEST,
        "Error code should be INVALID_REQUEST"
    );
}

/// Test that `qdrant_deprecate` rejects requests when server is in read-only mode.
#[rstest]
#[tokio::test]
async fn test_read_only_mode_rejects_deprecate(mut settings: Settings) {
    settings.qdrant.read_only = true;
    let connector = MockQdrantConnector::new();

    let server = DiaryServer::new(connector, settings);
    let request = DeprecateRequest {
        query: "test query".to_owned(),
        collection_name: None,
    };

    let result = server.qdrant_deprecate(Parameters(request)).await;

    let err = result.expect_err("Deprecate should fail in read-only mode");
    assert_eq!(
        err.code,
        ErrorCode::INVALID_REQUEST,
        "Error code should be INVALID_REQUEST"
    );
}

/// Test that `qdrant_store` succeeds with mocked connector.
#[rstest]
#[tokio::test]
async fn test_store_happy_path(settings: Settings) {
    let mut connector = MockQdrantConnector::new();

    connector.expect_store().returning(|_, _| Ok(()));

    let server = DiaryServer::new(connector, settings);
    let request = StoreRequest {
        information: "Test content to store".to_owned(),
        metadata: None,
        collection_name: None,
    };

    let result = server
        .qdrant_store(Parameters(request))
        .await
        .expect("qdrant_store should succeed");

    let content = result.content.first().expect("Should have content");
    let text = content_text(content);
    assert!(
        text.contains("Successfully stored"),
        "Success message should indicate successful storage"
    );
}

/// Test that `qdrant_find` returns error when backend fails.
#[rstest]
#[tokio::test]
async fn test_find_with_backend_error(settings: Settings) {
    let mut connector = MockQdrantConnector::new();

    connector.expect_collection_exists().returning(|_| Ok(true));

    connector
        .expect_search()
        .returning(|_, _| Err(QdrantError::SearchError("Backend failure".to_owned())));

    let server = DiaryServer::new(connector, settings);
    let request = FindRequest {
        query: "test".to_owned(),
        collection_name: None,
        filter: None,
        include_deprecated: false,
    };

    let result = server.qdrant_find(Parameters(request)).await;

    let err = result.expect_err("qdrant_find should fail with backend error");
    assert!(
        err.message.contains("Backend failure"),
        "Error message should contain backend failure details"
    );
}

/// Test that `qdrant_deprecate` handles missing collection gracefully.
#[rstest]
#[tokio::test]
async fn test_deprecate_collection_missing(settings: Settings) {
    let mut connector = MockQdrantConnector::new();

    connector
        .expect_collection_exists()
        .returning(|_| Ok(false));

    let server = DiaryServer::new(connector, settings);
    let request = DeprecateRequest {
        query: "test query".to_owned(),
        collection_name: None,
    };

    let result = server
        .qdrant_deprecate(Parameters(request))
        .await
        .expect("qdrant_deprecate should succeed with missing collection");

    let content = result.content.first().expect("Should have content");
    let text = content_text(content);
    assert!(
        text.contains("doesn't exist yet"),
        "Message should indicate collection doesn't exist yet"
    );
}
