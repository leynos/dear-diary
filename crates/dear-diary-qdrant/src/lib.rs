//! Qdrant connector for Dear Diary MCP server.
//!
//! This crate provides the [`QdrantConnector`] for interacting with
//! the Qdrant vector database, including storing and searching entries.
//!
//! # Example
//!
//! ```ignore
//! use dear_diary_qdrant::{Entry, QdrantConnector, QdrantConnectorImpl};
//! use dear_diary_embeddings::FastEmbedProvider;
//!
//! let embedding_provider = FastEmbedProvider::new("sentence-transformers/all-MiniLM-L6-v2")?;
//! let connector = QdrantConnectorImpl::new(
//!     "http://localhost:6334",
//!     None,
//!     None,
//!     embedding_provider,
//! ).await?;
//!
//! let entry = Entry::new("Remember to buy milk");
//! connector.store(&entry, "memories").await?;
//! ```

mod connector;
mod entry;
mod error;

pub use connector::{QdrantConnector, QdrantConnectorImpl};
pub use entry::{Entry, SearchResult};
pub use error::QdrantError;

// Export mock for testing in dependent crates
#[cfg(any(test, feature = "test-utils"))]
pub use connector::MockQdrantConnector;
