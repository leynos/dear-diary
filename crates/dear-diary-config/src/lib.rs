//! Configuration management for Dear Diary MCP server.
//!
//! This crate provides configuration types and loading mechanisms using
//! `ortho_config` for unified handling of environment variables,
//! configuration files, and command-line arguments.
//!
//! # Environment Variables
//!
//! | Variable | Description | Default |
//! |----------|-------------|---------|
//! | `QDRANT_URL` | Qdrant server URL | None |
//! | `QDRANT_API_KEY` | API key for Qdrant | None |
//! | `COLLECTION_NAME` | Default collection name (supports interpolation) | None |
//! | `QDRANT_LOCAL_PATH` | Local storage path | None |
//! | `EMBEDDING_MODEL` | Embedding model name | `sentence-transformers/all-MiniLM-L6-v2` |
//! | `QDRANT_SEARCH_LIMIT` | Max search results | 10 |
//! | `QDRANT_READ_ONLY` | Disable store tool | false |
//! | `TOOL_STORE_DESCRIPTION` | Custom store tool description | (default) |
//! | `TOOL_FIND_DESCRIPTION` | Custom find tool description | (default) |

mod error;
mod interpolation;
mod settings;

pub use error::ConfigError;
pub use interpolation::{GitContext, RealGitContext};
pub use settings::{
    DEFAULT_EMBEDDING_MODEL, FilterableField, FilterableFieldCondition, FilterableFieldType,
    QdrantSettings, Settings, ToolSettings,
};
