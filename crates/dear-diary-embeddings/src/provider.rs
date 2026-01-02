//! Embedding provider trait definition.

use crate::error::EmbeddingError;

#[cfg(test)]
use mockall::automock;

/// Trait for embedding providers that can convert text to vectors.
///
/// Embedding providers are responsible for transforming textual content
/// into dense vector representations suitable for semantic search.
#[cfg_attr(test, automock)]
pub trait EmbeddingProvider: Send + Sync {
    /// Embeds a batch of documents into vectors.
    ///
    /// # Arguments
    ///
    /// * `documents` - Slice of document strings to embed.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding fails.
    fn embed_documents(&self, documents: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Embeds a single query into a vector.
    ///
    /// Query embeddings may differ from document embeddings depending on
    /// the model (e.g., asymmetric models have different encoders for
    /// queries vs passages).
    ///
    /// # Arguments
    ///
    /// * `query` - The query string to embed.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding fails.
    fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Returns the name of the vector for Qdrant collection configuration.
    ///
    /// This is used when creating collections to name the vector field.
    fn vector_name(&self) -> String;

    /// Returns the dimensionality of the embedding vectors.
    ///
    /// This is used when creating collections to configure vector size.
    fn vector_size(&self) -> usize;
}
