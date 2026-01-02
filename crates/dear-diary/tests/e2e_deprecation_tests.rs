//! End-to-end tests for the deprecation feature.
//!
//! Tests marking entries as deprecated and verifying visibility behaviour.
//!
//! Note: Tests for entries deprecated >7 days (hidden by default, visible with
//! `include_deprecated=true`) are implemented with mocks in
//! `dear_diary_mcp::server_tests` since e2e tests cannot practically wait 7+ days.
//! See `test_old_deprecated_entries_hidden_in_find` and
//! `test_old_deprecated_entries_visible_with_flag`.

mod common;

use std::time::Duration;

use dear_diary_qdrant::{Entry, QdrantConnector, SearchQuery};
use rstest::rstest;

use crate::common::{TestFixture, wait_for_indexing};

/// Feature: Deprecating memories
/// Scenario: Deprecate an entry and verify it's marked as deprecated
#[rstest]
#[tokio::test]
async fn test_deprecate_entry() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("deprecate_entry").expect("Failed to create test fixture");

    let entry = Entry::new("Remember to call the dentist");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    wait_for_indexing(
        &fixture.connector,
        &fixture.collection_name,
        "dentist",
        Duration::from_secs(5),
    )
    .await
    .expect("Indexing should complete");

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

    fixture
        .connector
        .deprecate(&result.point_id, &fixture.collection_name)
        .await
        .expect("Deprecate should succeed");

    // Small delay for deprecation to be persisted
    tokio::time::sleep(Duration::from_millis(100)).await;

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

    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Deprecating memories
/// Scenario: Recently deprecated entries are still visible but flagged
#[rstest]
#[tokio::test]
async fn test_deprecated_entry_visible_within_7_days() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("deprecated_visible").expect("Failed to create test fixture");

    let entry = Entry::new("Pick up dry cleaning");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    wait_for_indexing(
        &fixture.connector,
        &fixture.collection_name,
        "dry cleaning",
        Duration::from_secs(5),
    )
    .await
    .expect("Indexing should complete");

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

    // Small delay for deprecation to be persisted
    tokio::time::sleep(Duration::from_millis(100)).await;

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

    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Deprecating memories
/// Scenario: `SearchResult` includes `point_id` for subsequent operations
#[rstest]
#[tokio::test]
async fn test_search_result_includes_point_id() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("point_id").expect("Failed to create test fixture");

    let entry = Entry::new("Test entry for point ID verification");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    wait_for_indexing(
        &fixture.connector,
        &fixture.collection_name,
        "point ID",
        Duration::from_secs(5),
    )
    .await
    .expect("Indexing should complete");

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

    fixture.cleanup().await.expect("Cleanup failed");
}
