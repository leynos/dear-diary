//! MCP server error types.

use thiserror::Error;

/// Errors that can occur during MCP server operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpServerError {
    /// Qdrant operation failed.
    #[error("Qdrant error: {0}")]
    Qdrant(#[from] dear_diary_qdrant::QdrantError),

    /// Collection name was required but not provided.
    #[error("Collection name is required (no default configured)")]
    MissingCollectionName,

    /// Server is in read-only mode.
    #[error("Server is in read-only mode, store operations are disabled")]
    ReadOnlyMode,

    /// Invalid filter provided.
    #[error("Invalid filter: {0}")]
    InvalidFilter(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}
