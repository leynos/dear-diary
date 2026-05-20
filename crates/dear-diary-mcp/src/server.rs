//! Dear Diary MCP server implementation.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use qdrant_client::qdrant::Filter;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde_json::Value;

use dear_diary_config::Settings;
use dear_diary_qdrant::{Entry, QdrantConnector, SearchQuery};

use crate::deprecation::visibility as deprecation_visibility;
use crate::error::{McpServerError, internal_error, invalid_params, read_only_error};
use crate::tools::{DeprecateRequest, FindRequest, StoreRequest};

/// The Dear Diary MCP server.
///
/// This server provides tools for storing and retrieving information
/// from a Qdrant vector database.
#[derive(Clone)]
pub struct DiaryServer<C: QdrantConnector> {
    connector: Arc<C>,
    settings: Arc<Settings>,
    tool_router: ToolRouter<Self>,
}

impl<C: QdrantConnector + 'static> DiaryServer<C> {
    /// Creates a new Dear Diary server.
    ///
    /// # Arguments
    ///
    /// * `connector` - The Qdrant connector to use.
    /// * `settings` - The server settings.
    #[must_use]
    pub fn new(connector: C, settings: Settings) -> Self {
        let tool_router = Self::tool_router();
        Self {
            connector: Arc::new(connector),
            settings: Arc::new(settings),
            tool_router,
        }
    }

    /// Returns the collection name to use, resolving defaults.
    fn resolve_collection_name(&self, provided: Option<&str>) -> Result<String, McpServerError> {
        provided
            .map(String::from)
            .or_else(|| self.settings.qdrant.collection_name.clone())
            .ok_or(McpServerError::MissingCollectionName)
    }

    /// Converts an optional JSON filter to a Qdrant filter.
    ///
    /// Note: Arbitrary filter support is limited. Currently, only filterable
    /// fields are properly supported. Raw JSON filter parsing is not available
    /// because the Qdrant protobuf Filter type doesn't implement serde traits.
    ///
    /// See [GitHub issue #2][filter-issue] for implementation tracking and the
    /// [user guide][user-guide-filters] for current workarounds.
    ///
    /// [filter-issue]: https://github.com/leynos/dear-diary/issues/2
    /// [user-guide-filters]: https://github.com/leynos/dear-diary/blob/main/docs/users-guide.md#arbitrary-filter-parsing
    fn parse_filter(&self, filter: Option<Value>) -> Result<Option<Filter>, McpServerError> {
        let Some(_filter_value) = filter else {
            return Ok(None);
        };

        if !self.settings.qdrant.allow_arbitrary_filter {
            return Err(McpServerError::InvalidFilter(
                "Arbitrary filters are not enabled".to_owned(),
            ));
        }

        // Arbitrary JSON filter parsing is not yet implemented.
        // The Qdrant Filter protobuf type doesn't implement serde Deserialize,
        // so we would need to manually construct filters from the JSON structure.
        // Tracked in: https://github.com/leynos/dear-diary/issues/2
        Err(McpServerError::InvalidFilter(
            "Arbitrary JSON filter parsing is not yet implemented".to_owned(),
        ))
    }
}

#[tool_router]
impl<C: QdrantConnector + 'static> DiaryServer<C> {
    /// Store information in the Qdrant database.
    #[tool(description = "Store information in the Qdrant database")]
    async fn qdrant_store(
        &self,
        Parameters(request): Parameters<StoreRequest>,
    ) -> Result<CallToolResult, McpError> {
        if self.settings.qdrant.read_only {
            return Err(read_only_error());
        }

        let collection_name = self
            .resolve_collection_name(request.collection_name.as_deref())
            .map_err(invalid_params)?;

        let entry = match request.metadata {
            Some(metadata) => Entry::with_metadata(request.information, metadata),
            None => Entry::new(request.information),
        };

        self.connector
            .store(&entry, &collection_name)
            .await
            .map_err(internal_error)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Successfully stored information in collection '{collection_name}'"
        ))]))
    }

    /// Find relevant information from the Qdrant database.
    #[tool(description = "Find relevant information from the Qdrant database")]
    async fn qdrant_find(
        &self,
        Parameters(request): Parameters<FindRequest>,
    ) -> Result<CallToolResult, McpError> {
        let collection_name = self
            .resolve_collection_name(request.collection_name.as_deref())
            .map_err(invalid_params)?;

        let filter = self.parse_filter(request.filter).map_err(invalid_params)?;

        let collection_exists = self
            .connector
            .collection_exists(&collection_name)
            .await
            .map_err(|e| internal_error(format!("Failed to check collection existence: {e}")))?;

        if !collection_exists {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "The collection '{collection_name}' doesn't exist yet. \
                 Use qdrant_store to add some information first."
            ))]));
        }

        let limit = u64::from(self.settings.qdrant.search_limit);
        let search_query = match filter {
            Some(f) => SearchQuery::with_filter(&request.query, limit, f),
            None => SearchQuery::new(&request.query, limit),
        };
        let results = self
            .connector
            .search(&search_query, &collection_name)
            .await
            .map_err(internal_error)?;

        // Get current time for deprecation filtering
        #[expect(clippy::cast_possible_wrap, reason = "Unix timestamp fits in i64")]
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .map_err(|e| internal_error(format!("System time before Unix epoch: {e}")))?;

        // Filter and format results based on deprecation state
        let content: Vec<Content> = results
            .into_iter()
            .filter_map(|result| {
                let visibility =
                    deprecation_visibility(now, result.deprecated_at, request.include_deprecated);

                if !visibility.include {
                    return None;
                }

                // Format the entry text
                let prefix = if visibility.is_deprecated {
                    "[DEPRECATED] "
                } else {
                    ""
                };
                let text = match result.entry.metadata {
                    Some(metadata) => {
                        let metadata_json = serde_json::to_string_pretty(&metadata)
                            .unwrap_or_else(|_| "{}".to_owned());
                        format!(
                            "{prefix}{}\n\nMetadata: {metadata_json}",
                            result.entry.content
                        )
                    }
                    None => format!("{prefix}{}", result.entry.content),
                };
                Some(Content::text(text))
            })
            .collect();

        // Check if we have any results after filtering
        if content.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "No results found in '{collection_name}' matching your query. \
                 Try a different search term or add more information with qdrant_store."
            ))]));
        }

        Ok(CallToolResult::success(content))
    }

    /// Deprecate an entry in the Qdrant database.
    #[tool(description = "Mark an entry as deprecated (it will be hidden after 7 days)")]
    async fn qdrant_deprecate(
        &self,
        Parameters(request): Parameters<DeprecateRequest>,
    ) -> Result<CallToolResult, McpError> {
        if self.settings.qdrant.read_only {
            return Err(read_only_error());
        }

        let collection_name = self
            .resolve_collection_name(request.collection_name.as_deref())
            .map_err(invalid_params)?;

        let collection_exists = self
            .connector
            .collection_exists(&collection_name)
            .await
            .map_err(|e| internal_error(format!("Failed to check collection existence: {e}")))?;

        if !collection_exists {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "The collection '{collection_name}' doesn't exist yet. \
                 Nothing to deprecate."
            ))]));
        }

        let search_query = SearchQuery::new(&request.query, 1);
        let results = self
            .connector
            .search(&search_query, &collection_name)
            .await
            .map_err(internal_error)?;

        // Get the top result
        let Some(top_result) = results.into_iter().next() else {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "No matching entry found in '{collection_name}' for query: {}",
                request.query
            ))]));
        };

        // Check if already deprecated
        if top_result.deprecated_at.is_some() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Entry is already deprecated: {}",
                top_result.entry.content
            ))]));
        }

        self.connector
            .deprecate(&top_result.point_id, &collection_name)
            .await
            .map_err(internal_error)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Successfully deprecated entry: {}",
            top_result.entry.content
        ))]))
    }
}

#[tool_handler]
impl<C: QdrantConnector + 'static> ServerHandler for DiaryServer<C> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "dear-diary".to_owned(),
                title: Some("Dear Diary MCP Server".to_owned()),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                icons: None,
                website_url: Some("https://github.com/leynos/dear-diary".to_owned()),
            },
            instructions: Some(
                "Dear Diary is an MCP server for storing and retrieving information \
                 using semantic search powered by Qdrant vector database. \
                 Use qdrant_store to save information and qdrant_find to search for it."
                    .to_owned(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for server construction and collection-name resolution.

    use super::*;
    use dear_diary_config::{DEFAULT_EMBEDDING_MODEL, QdrantSettings, ToolSettings};
    use dear_diary_qdrant::MockQdrantConnector;
    use rstest::{fixture, rstest};

    #[fixture]
    pub(super) fn settings() -> Settings {
        Settings {
            tools: ToolSettings::default(),
            qdrant: QdrantSettings {
                qdrant_url: Some("http://localhost:6334".to_owned()),
                qdrant_api_key: None,
                collection_name: Some("test_collection".to_owned()),
                qdrant_local_path: None,
                search_limit: 10,
                read_only: false,
                filterable_fields: Vec::new(),
                allow_arbitrary_filter: false,
            },
            embedding_model: DEFAULT_EMBEDDING_MODEL.to_owned(),
        }
    }

    #[rstest]
    fn test_server_creation(settings: Settings) {
        let connector = MockQdrantConnector::new();
        let _server = DiaryServer::new(connector, settings);
    }

    #[rstest]
    fn test_resolve_collection_name_with_provided(settings: Settings) {
        let connector = MockQdrantConnector::new();
        let server = DiaryServer::new(connector, settings);

        let result = server
            .resolve_collection_name(Some("custom_collection"))
            .expect("resolve_collection_name should succeed");
        assert_eq!(result, "custom_collection");
    }

    #[rstest]
    fn test_resolve_collection_name_with_default(settings: Settings) {
        let connector = MockQdrantConnector::new();
        let server = DiaryServer::new(connector, settings);

        let result = server
            .resolve_collection_name(None)
            .expect("resolve_collection_name should succeed");
        assert_eq!(result, "test_collection");
    }

    #[rstest]
    fn test_resolve_collection_name_missing(mut settings: Settings) {
        let connector = MockQdrantConnector::new();
        settings.qdrant.collection_name = None;
        let server = DiaryServer::new(connector, settings);

        let result = server.resolve_collection_name(None);
        assert!(result.is_err());
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod server_tests;
