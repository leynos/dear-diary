//! End-to-end tests for the store feature.
//!
//! Tests storing information in the Qdrant vector database.

mod common;

use std::time::Duration;

use dear_diary_qdrant::{Entry, QdrantConnector, SearchQuery};
use rstest::rstest;
use serde_json::json;

use crate::common::{TestFixture, wait_for_indexing};

/// Feature: Storing information in Qdrant
/// Scenario: Store and retrieve a simple memory
#[rstest]
#[tokio::test]
async fn test_store_and_retrieve_simple_memory() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("store_simple").expect("Failed to create test fixture");

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

    wait_for_indexing(
        &fixture.connector,
        &fixture.collection_name,
        "milk",
        Duration::from_secs(5),
    )
    .await
    .expect("Indexing should complete");

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

    fixture.cleanup().await.expect("Cleanup failed");
}

/// Feature: Storing information in Qdrant
/// Scenario: Store information with metadata
#[rstest]
#[tokio::test]
async fn test_store_with_metadata() {
    skip_unless_e2e!();
    let fixture = TestFixture::new("store_metadata").expect("Failed to create test fixture");

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

    wait_for_indexing(
        &fixture.connector,
        &fixture.collection_name,
        "meeting",
        Duration::from_secs(5),
    )
    .await
    .expect("Indexing should complete");

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

    let first_result = results.first().expect("Should have at least one result");
    assert!(
        first_result.entry.metadata.is_some(),
        "Result should have metadata"
    );
    assert!(
        first_result.entry.content.contains("meeting"),
        "Result should contain stored content"
    );

    fixture.cleanup().await.expect("Cleanup failed");
}
