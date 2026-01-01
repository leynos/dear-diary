//! `FastEmbed` implementation of the embedding provider.

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::error::EmbeddingError;
use crate::provider::EmbeddingProvider;

/// Embedding provider using `FastEmbed` for local model inference.
///
/// `FastEmbed` provides efficient CPU-based inference for embedding models,
/// eliminating the need for external API calls.
pub struct FastEmbedProvider {
    model: TextEmbedding,
    model_name: String,
    vector_size: usize,
}

impl FastEmbedProvider {
    /// Creates a new `FastEmbedProvider` with the specified model.
    ///
    /// # Arguments
    ///
    /// * `model_name` - The name of the embedding model (e.g.,
    ///   `sentence-transformers/all-MiniLM-L6-v2`).
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be initialized.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = FastEmbedProvider::new("sentence-transformers/all-MiniLM-L6-v2")?;
    /// ```
    pub fn new(model_name: &str) -> Result<Self, EmbeddingError> {
        // Map model name to FastEmbed's EmbeddingModel enum
        let embedding_model = Self::parse_model_name(model_name)?;

        // Get vector size before moving embedding_model
        let vector_size = Self::get_model_dimension(&embedding_model);

        let model = TextEmbedding::try_new(InitOptions::new(embedding_model).with_show_download_progress(false))
            .map_err(|e| EmbeddingError::InitializationError(e.to_string()))?;

        Ok(Self {
            model,
            model_name: model_name.to_owned(),
            vector_size,
        })
    }

    /// Parses a model name string into a `FastEmbed` `EmbeddingModel`.
    fn parse_model_name(model_name: &str) -> Result<EmbeddingModel, EmbeddingError> {
        // FastEmbed supports various models; map common names
        match model_name {
            "sentence-transformers/all-MiniLM-L6-v2" | "all-MiniLM-L6-v2" => {
                Ok(EmbeddingModel::AllMiniLML6V2)
            }
            "BAAI/bge-small-en-v1.5" | "bge-small-en-v1.5" => Ok(EmbeddingModel::BGESmallENV15),
            "BAAI/bge-base-en-v1.5" | "bge-base-en-v1.5" => Ok(EmbeddingModel::BGEBaseENV15),
            "BAAI/bge-large-en-v1.5" | "bge-large-en-v1.5" => Ok(EmbeddingModel::BGELargeENV15),
            _ => Err(EmbeddingError::InitializationError(format!(
                "Unsupported embedding model: {model_name}. \
                 Supported models: sentence-transformers/all-MiniLM-L6-v2, \
                 BAAI/bge-small-en-v1.5, BAAI/bge-base-en-v1.5, BAAI/bge-large-en-v1.5"
            ))),
        }
    }

    /// Returns the vector dimension for a given model.
    const fn get_model_dimension(model: &EmbeddingModel) -> usize {
        match model {
            EmbeddingModel::AllMiniLML6V2 => 384,
            EmbeddingModel::BGESmallENV15 => 384,
            EmbeddingModel::BGEBaseENV15 => 768,
            EmbeddingModel::BGELargeENV15 => 1024,
            _ => 384, // Default fallback
        }
    }
}

impl EmbeddingProvider for FastEmbedProvider {
    fn embed_documents(&self, documents: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.model
            .embed(documents, None)
            .map_err(|e| EmbeddingError::EmbedError(e.to_string()))
    }

    fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError> {
        let embeddings = self
            .model
            .embed(vec![query.to_owned()], None)
            .map_err(|e| EmbeddingError::QueryEmbedError(e.to_string()))?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::QueryEmbedError("No embedding returned".to_owned()))
    }

    fn vector_name(&self) -> String {
        // Extract the model name suffix and create a vector name compatible with FastEmbed
        let model_suffix = self
            .model_name
            .rsplit('/')
            .next()
            .unwrap_or(&self.model_name)
            .to_lowercase();
        format!("fast-{model_suffix}")
    }

    fn vector_size(&self) -> usize {
        self.vector_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_parse_valid_model_names() {
        assert!(FastEmbedProvider::parse_model_name("sentence-transformers/all-MiniLM-L6-v2").is_ok());
        assert!(FastEmbedProvider::parse_model_name("all-MiniLM-L6-v2").is_ok());
        assert!(FastEmbedProvider::parse_model_name("BAAI/bge-small-en-v1.5").is_ok());
    }

    #[rstest]
    fn test_parse_invalid_model_name() {
        let result = FastEmbedProvider::parse_model_name("unknown-model");
        assert!(result.is_err());
    }

    #[rstest]
    fn test_model_dimensions() {
        assert_eq!(FastEmbedProvider::get_model_dimension(&EmbeddingModel::AllMiniLML6V2), 384);
        assert_eq!(FastEmbedProvider::get_model_dimension(&EmbeddingModel::BGEBaseENV15), 768);
        assert_eq!(FastEmbedProvider::get_model_dimension(&EmbeddingModel::BGELargeENV15), 1024);
    }
}
