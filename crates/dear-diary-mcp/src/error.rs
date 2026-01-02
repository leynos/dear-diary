//! MCP server error types.

use std::borrow::Cow;
use std::fmt::Display;

use rmcp::ErrorData as McpError;
use rmcp::model::ErrorCode;
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

    /// Serialisation error.
    #[error("Serialisation error: {0}")]
    Serialisation(String),
}

/// Creates an MCP error with `INVALID_PARAMS` code.
pub(crate) fn invalid_params(e: impl Display) -> McpError {
    McpError {
        code: ErrorCode::INVALID_PARAMS,
        message: Cow::Owned(e.to_string()),
        data: None,
    }
}

/// Creates an MCP error with `INTERNAL_ERROR` code.
pub(crate) fn internal_error(e: impl Display) -> McpError {
    McpError {
        code: ErrorCode::INTERNAL_ERROR,
        message: Cow::Owned(e.to_string()),
        data: None,
    }
}

/// Creates an MCP error for read-only mode.
pub(crate) const fn read_only_error() -> McpError {
    McpError {
        code: ErrorCode::INVALID_REQUEST,
        message: Cow::Borrowed("Server is in read-only mode"),
        data: None,
    }
}
