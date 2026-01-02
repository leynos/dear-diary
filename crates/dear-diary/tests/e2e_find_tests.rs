//! End-to-end tests for the find feature.
//!
//! Tests searching and retrieving information from the Qdrant vector database.

mod common;

use dear_diary_qdrant::{Entry, QdrantConnector, SearchQuery};
use rstest::rstest;

use crate::common::TestFixture;

/// Feature: Finding information in Qdrant
/// Scenario: Search in empty/non-existent collection returns no results
#[rstest]
#[tokio::test]
async fn test_search_empty_collection() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("search_empty").expect("Failed to create test fixture");

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

    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Finding information in Qdrant
/// Scenario: Semantic search returns relevant results
#[rstest]
#[tokio::test]
async fn test_semantic_search() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("semantic_search").expect("Failed to create test fixture");

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

    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

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

    let first_content = &results.first().expect("Should have results").entry.content;
    let has_pets = first_content.contains("cat") || first_content.contains("dog");
    assert!(has_pets, "Semantic search should find pet-related entries");

    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Finding information in Qdrant
/// Scenario: Search limit is respected
#[rstest]
#[tokio::test]
async fn test_search_limit() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("search_limit").expect("Failed to create test fixture");

    for i in 0..5 {
        let entry = Entry::new(format!("Test memory number {i}"));
        fixture
            .connector
            .store(&entry, &fixture.collection_name)
            .await
            .expect("Store should succeed");
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

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

    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Collection existence check
/// Scenario: Collection exists after storing data
#[rstest]
#[tokio::test]
async fn test_collection_exists_after_store() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("collection_exists").expect("Failed to create test fixture");

    let exists_before = fixture
        .connector
        .collection_exists(&fixture.collection_name)
        .await
        .expect("Collection check should succeed");

    assert!(!exists_before, "Collection should not exist initially");

    let entry = Entry::new("Test entry");
    fixture
        .connector
        .store(&entry, &fixture.collection_name)
        .await
        .expect("Store should succeed");

    let exists_after = fixture
        .connector
        .collection_exists(&fixture.collection_name)
        .await
        .expect("Collection check should succeed");

    assert!(exists_after, "Collection should exist after storing");

    fixture.cleanup().await.expect("Cleanup failed");
}
