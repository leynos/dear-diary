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

    /// A placeholder in `COLLECTION_NAME` could not be resolved.
    #[error("Cannot resolve placeholder {{{placeholder}}} in COLLECTION_NAME: {reason}")]
    UnresolvablePlaceholder {
        /// The placeholder name (e.g. "repo", "owner").
        placeholder: String,
        /// Human-readable explanation of why resolution failed.
        reason: String,
    },

    /// Failed to construct the interpolation context for `COLLECTION_NAME`.
    ///
    /// Used for errors not caused by git commands, such as URL parsing
    /// failures or determining the current working directory.
    #[error("Failed during COLLECTION_NAME interpolation: {0}")]
    InterpolationContextError(String),

    /// A git command failed during placeholder resolution.
    ///
    /// Reserved for errors originating from the git CLI.
    #[error("Git command failed during COLLECTION_NAME interpolation: {0}")]
    GitCommandError(String),
}
