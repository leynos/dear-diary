//! Embedding provider abstractions for Dear Diary MCP server.
//!
//! This crate defines the [`EmbeddingProvider`] trait and provides
//! implementations for local embedding models via `fastembed`.
//!
//! # Example
//!
//! ```ignore
//! use dear_diary_embeddings::{EmbeddingProvider, FastEmbedProvider};
//!
//! let provider = FastEmbedProvider::new("sentence-transformers/all-MiniLM-L6-v2")?;
//! let embeddings = provider.embed_documents(vec!["Hello, world!".to_owned()])?;
//! ```

mod error;
mod fastembed;
mod provider;

pub use error::EmbeddingError;
pub use fastembed::FastEmbedProvider;
pub use provider::EmbeddingProvider;

#[cfg(test)]
pub use provider::MockEmbeddingProvider;
