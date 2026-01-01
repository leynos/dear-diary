//! Embedding error types.

use thiserror::Error;

/// Errors that can occur during embedding operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EmbeddingError {
    /// Failed to initialize the embedding model.
    #[error("Failed to initialize embedding model: {0}")]
    InitializationError(String),

    /// Failed to embed documents.
    #[error("Failed to embed documents: {0}")]
    EmbedError(String),

    /// Failed to embed query.
    #[error("Failed to embed query: {0}")]
    QueryEmbedError(String),
}
