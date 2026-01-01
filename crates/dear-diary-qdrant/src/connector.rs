//! Qdrant connector trait and implementation.

use std::sync::Arc;

use std::time::{SystemTime, UNIX_EPOCH};

use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, PointsIdsList, SearchPointsBuilder,
    SetPayloadPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder, VectorsConfig,
};
use qdrant_client::{Payload, Qdrant};
use serde_json::json;

use dear_diary_embeddings::EmbeddingProvider;

use crate::entry::{Entry, SearchQuery, SearchResult};
use crate::error::QdrantError;

#[cfg(any(test, feature = "test-utils"))]
use mockall::automock;

/// Trait for Qdrant database operations.
///
/// This trait abstracts the Qdrant client to allow for mocking in tests.
#[cfg_attr(any(test, feature = "test-utils"), automock)]
pub trait QdrantConnector: Send + Sync {
    /// Stores an entry in the specified collection.
    ///
    /// # Arguments
    ///
    /// * `entry` - The entry to store.
    /// * `collection_name` - The name of the collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn store(
        &self,
        entry: &Entry,
        collection_name: &str,
    ) -> impl std::future::Future<Output = Result<(), QdrantError>> + Send;

    /// Searches for entries matching the query.
    ///
    /// # Arguments
    ///
    /// * `search_query` - The search parameters including query text, limit, and filter.
    /// * `collection_name` - The name of the collection to search.
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    fn search(
        &self,
        search_query: &SearchQuery,
        collection_name: &str,
    ) -> impl std::future::Future<Output = Result<Vec<SearchResult>, QdrantError>> + Send;

    /// Checks if a collection exists.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - The name of the collection to check.
    ///
    /// # Errors
    ///
    /// Returns an error if the check fails.
    fn collection_exists(
        &self,
        collection_name: &str,
    ) -> impl std::future::Future<Output = Result<bool, QdrantError>> + Send;

    /// Marks an entry as deprecated by setting its `deprecated_at` timestamp.
    ///
    /// # Arguments
    ///
    /// * `point_id` - The Qdrant point ID of the entry to deprecate.
    /// * `collection_name` - The name of the collection containing the entry.
    ///
    /// # Errors
    ///
    /// Returns an error if the deprecation operation fails.
    fn deprecate(
        &self,
        point_id: &str,
        collection_name: &str,
    ) -> impl std::future::Future<Output = Result<(), QdrantError>> + Send;
}

/// Implementation of [`QdrantConnector`] using the Qdrant client.
pub struct QdrantConnectorImpl<E: EmbeddingProvider> {
    client: Qdrant,
    embedding_provider: Arc<E>,
    default_collection: Option<String>,
}

impl<E: EmbeddingProvider> QdrantConnectorImpl<E> {
    /// Creates a new connector with a remote Qdrant server.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the Qdrant server.
    /// * `api_key` - Optional API key for authentication.
    /// * `default_collection` - Optional default collection name.
    /// * `embedding_provider` - The embedding provider to use.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established.
    pub fn new(
        url: &str,
        api_key: Option<&str>,
        default_collection: Option<String>,
        embedding_provider: E,
    ) -> Result<Self, QdrantError> {
        let mut builder = Qdrant::from_url(url);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }

        let client = builder
            .build()
            .map_err(|e| QdrantError::ConnectionError(e.to_string()))?;

        Ok(Self {
            client,
            embedding_provider: Arc::new(embedding_provider),
            default_collection,
        })
    }

    /// Creates a new connector with a local Qdrant storage path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the local storage directory.
    /// * `default_collection` - Optional default collection name.
    /// * `embedding_provider` - The embedding provider to use.
    ///
    /// # Errors
    ///
    /// Returns an error if the local database cannot be initialized.
    pub fn new_local(
        path: &str,
        default_collection: Option<String>,
        embedding_provider: E,
    ) -> Result<Self, QdrantError> {
        // For local mode, we use the special `:memory:` URL or file path
        let client = if path == ":memory:" {
            Qdrant::from_url("http://localhost:6334")
                .build()
                .map_err(|e| QdrantError::ConnectionError(e.to_string()))?
        } else {
            Qdrant::from_url(&format!("file://{path}"))
                .build()
                .map_err(|e| QdrantError::ConnectionError(e.to_string()))?
        };

        Ok(Self {
            client,
            embedding_provider: Arc::new(embedding_provider),
            default_collection,
        })
    }

    /// Returns the default collection name if configured.
    #[must_use]
    pub const fn default_collection(&self) -> Option<&String> {
        self.default_collection.as_ref()
    }

    /// Ensures the collection exists, creating it if necessary.
    async fn ensure_collection_exists(&self, collection_name: &str) -> Result<(), QdrantError> {
        let exists = self
            .client
            .collection_exists(collection_name)
            .await
            .map_err(|e| QdrantError::ConnectionError(e.to_string()))?;

        if !exists {
            let vector_size = self.embedding_provider.vector_size();
            let vector_name = self.embedding_provider.vector_name();

            let vectors_config = VectorsConfig::from(VectorParamsBuilder::new(
                vector_size.try_into().map_err(|_| {
                    QdrantError::CreateCollectionError("Vector size too large".to_owned())
                })?,
                Distance::Cosine,
            ));

            self.client
                .create_collection(
                    CreateCollectionBuilder::new(collection_name).vectors_config(vectors_config),
                )
                .await
                .map_err(|e| QdrantError::CreateCollectionError(e.to_string()))?;

            // Note: The vector_name is used when upserting points, but collection
            // creation with VectorsConfig creates a default unnamed vector.
            // If we need named vectors, we'd need to adjust the collection creation.
            let _ = vector_name; // Acknowledge unused for now
        }

        Ok(())
    }
}

impl<E: EmbeddingProvider + 'static> QdrantConnector for QdrantConnectorImpl<E> {
    async fn store(&self, entry: &Entry, collection_name: &str) -> Result<(), QdrantError> {
        self.ensure_collection_exists(collection_name).await?;

        // Embed the document
        let embeddings = self
            .embedding_provider
            .embed_documents(vec![entry.content.clone()])?;

        let embedding = embeddings.into_iter().next().ok_or_else(|| {
            QdrantError::EmbeddingError(dear_diary_embeddings::EmbeddingError::EmbedError(
                "No embedding returned".to_owned(),
            ))
        })?;

        // Generate a unique ID
        let point_id = uuid::Uuid::new_v4().to_string();

        // Build payload
        let payload_value = json!({
            "document": entry.content,
            "metadata": entry.metadata,
        });

        let payload =
            Payload::try_from(payload_value).map_err(|e| QdrantError::StoreError(e.to_string()))?;

        // Create point
        let point = PointStruct::new(point_id, embedding, payload);

        // Upsert to collection
        self.client
            .upsert_points(UpsertPointsBuilder::new(collection_name, vec![point]))
            .await
            .map_err(|e| QdrantError::StoreError(e.to_string()))?;

        Ok(())
    }

    async fn search(
        &self,
        search_query: &SearchQuery,
        collection_name: &str,
    ) -> Result<Vec<SearchResult>, QdrantError> {
        // Check if collection exists
        let exists = self
            .client
            .collection_exists(collection_name)
            .await
            .map_err(|e| QdrantError::ConnectionError(e.to_string()))?;

        if !exists {
            return Ok(Vec::new());
        }

        // Embed the query
        let query_vector = self.embedding_provider.embed_query(&search_query.query)?;

        // Build search request
        let mut search_builder =
            SearchPointsBuilder::new(collection_name, query_vector, search_query.limit)
                .with_payload(true);

        if let Some(ref f) = search_query.filter {
            search_builder = search_builder.filter(f.clone());
        }

        // Execute search
        let results = self
            .client
            .search_points(search_builder)
            .await
            .map_err(|e| QdrantError::SearchError(e.to_string()))?;

        // Convert results to SearchResult with point ID and deprecation state
        let search_results = results
            .result
            .into_iter()
            .filter_map(|point| {
                // Extract point ID
                let point_id = match point.id?.point_id_options? {
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid) => uuid,
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(num) => num.to_string(),
                };

                let payload = point.payload;
                let document = payload.get("document")?.as_str()?.to_owned();
                let metadata = payload
                    .get("metadata")
                    .and_then(|v| serde_json::from_value(v.clone().into_json()).ok());

                // Extract deprecated_at timestamp if present
                let deprecated_at = payload
                    .get("deprecated_at")
                    .and_then(qdrant_client::qdrant::Value::as_integer);

                Some(SearchResult {
                    point_id,
                    entry: Entry {
                        content: document,
                        metadata,
                    },
                    deprecated_at,
                })
            })
            .collect();

        Ok(search_results)
    }

    async fn collection_exists(&self, collection_name: &str) -> Result<bool, QdrantError> {
        self.client
            .collection_exists(collection_name)
            .await
            .map_err(|e| QdrantError::ConnectionError(e.to_string()))
    }

    async fn deprecate(&self, point_id: &str, collection_name: &str) -> Result<(), QdrantError> {
        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Build payload with deprecated_at timestamp
        // Unix timestamps in seconds fit comfortably in i64 until year 292 billion
        #[expect(clippy::cast_possible_wrap, reason = "Unix timestamp fits in i64")]
        let payload_value = json!({ "deprecated_at": now as i64 });
        let payload =
            Payload::try_from(payload_value).map_err(|e| QdrantError::StoreError(e.to_string()))?;

        // Update the point's payload
        self.client
            .set_payload(
                SetPayloadPointsBuilder::new(collection_name, payload).points_selector(
                    PointsIdsList {
                        ids: vec![point_id.to_owned().into()],
                    },
                ),
            )
            .await
            .map_err(|e| QdrantError::StoreError(e.to_string()))?;

        Ok(())
    }
}
