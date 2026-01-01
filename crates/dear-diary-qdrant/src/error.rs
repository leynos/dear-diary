//! Qdrant connector error types.

use thiserror::Error;

/// Errors that can occur during Qdrant operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum QdrantError {
    /// Failed to connect to Qdrant server.
    #[error("Failed to connect to Qdrant: {0}")]
    ConnectionError(String),

    /// Failed to create a collection.
    #[error("Failed to create collection: {0}")]
    CreateCollectionError(String),

    /// Failed to store an entry.
    #[error("Failed to store entry: {0}")]
    StoreError(String),

    /// Failed to search entries.
    #[error("Failed to search entries: {0}")]
    SearchError(String),

    /// Embedding operation failed.
    #[error("Embedding failed: {0}")]
    EmbeddingError(#[from] dear_diary_embeddings::EmbeddingError),

    /// Collection name was required but not provided.
    #[error("Collection name is required")]
    MissingCollectionName,
}
