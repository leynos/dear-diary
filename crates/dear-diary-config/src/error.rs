//! Configuration error types.

use thiserror::Error;

/// Errors that can occur during configuration loading or validation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// Both `qdrant_url` and `qdrant_local_path` were specified.
    #[error("Cannot specify both QDRANT_URL and QDRANT_LOCAL_PATH")]
    ConflictingConnectionModes,

    /// Neither `qdrant_url` nor `qdrant_local_path` was specified.
    #[error("Must specify either QDRANT_URL or QDRANT_LOCAL_PATH")]
    MissingConnectionConfig,

    /// Configuration loading failed.
    #[error("Failed to load configuration: {0}")]
    LoadError(String),
}
