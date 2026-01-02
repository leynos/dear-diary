//! Configuration settings for the Dear Diary MCP server.

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// Default description for the store tool.
pub const DEFAULT_TOOL_STORE_DESCRIPTION: &str =
    "Keep the memory for later use, when you are asked to remember something.";

/// Default description for the find tool.
pub const DEFAULT_TOOL_FIND_DESCRIPTION: &str = "\
Look up memories in Qdrant. Use this tool when you need to:
 - Find memories by their content
 - Access memories for further analysis
 - Get some personal information about the user";

/// Default embedding model name.
pub const DEFAULT_EMBEDDING_MODEL: &str = "sentence-transformers/all-MiniLM-L6-v2";

/// Default search result limit.
pub const DEFAULT_SEARCH_LIMIT: u32 = 10;

/// Tool-related settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolSettings {
    /// Description for the store tool shown to LLM clients.
    #[serde(default = "default_store_description")]
    pub tool_store_description: String,

    /// Description for the find tool shown to LLM clients.
    #[serde(default = "default_find_description")]
    pub tool_find_description: String,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            tool_store_description: DEFAULT_TOOL_STORE_DESCRIPTION.to_owned(),
            tool_find_description: DEFAULT_TOOL_FIND_DESCRIPTION.to_owned(),
        }
    }
}

fn default_store_description() -> String {
    DEFAULT_TOOL_STORE_DESCRIPTION.to_owned()
}

fn default_find_description() -> String {
    DEFAULT_TOOL_FIND_DESCRIPTION.to_owned()
}

/// Qdrant connection and behaviour settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QdrantSettings {
    /// URL of the Qdrant server (e.g., `http://localhost:6334`).
    pub qdrant_url: Option<String>,

    /// API key for authenticating with Qdrant.
    pub qdrant_api_key: Option<String>,

    /// Default collection name. If set, tools won't require `collection_name` parameter.
    pub collection_name: Option<String>,

    /// Path to local Qdrant storage directory (mutually exclusive with `qdrant_url`).
    pub qdrant_local_path: Option<String>,

    /// Maximum number of search results to return.
    #[serde(default = "default_search_limit")]
    pub search_limit: u32,

    /// When true, the store tool is disabled (read-only mode).
    #[serde(default)]
    pub read_only: bool,

    /// Fields that can be used for filtering search results.
    #[serde(default)]
    pub filterable_fields: Vec<FilterableField>,

    /// Whether to allow arbitrary filter expressions in search queries.
    #[serde(default)]
    pub allow_arbitrary_filter: bool,
}

impl Default for QdrantSettings {
    fn default() -> Self {
        Self {
            qdrant_url: None,
            qdrant_api_key: None,
            collection_name: None,
            qdrant_local_path: None,
            search_limit: DEFAULT_SEARCH_LIMIT,
            read_only: false,
            filterable_fields: Vec::new(),
            allow_arbitrary_filter: false,
        }
    }
}

impl QdrantSettings {
    /// Validates that exactly one connection mode is specified.
    ///
    /// # Errors
    ///
    /// Returns an error if both or neither of `qdrant_url` and `qdrant_local_path` are set.
    pub const fn validate(&self) -> Result<(), ConfigError> {
        // Use tuple matching to check both/neither/one of the connection options
        match (self.qdrant_url.is_some(), self.qdrant_local_path.is_some()) {
            (true, true) => Err(ConfigError::ConflictingConnectionModes),
            (false, false) => Err(ConfigError::MissingConnectionConfig),
            _ => Ok(()),
        }
    }

    /// Returns a map of filterable field names to their definitions.
    #[must_use]
    pub fn filterable_fields_map(&self) -> std::collections::HashMap<String, &FilterableField> {
        self.filterable_fields
            .iter()
            .map(|f| (f.name.clone(), f))
            .collect()
    }

    /// Returns filterable fields that have a condition defined.
    #[must_use]
    pub fn filterable_fields_with_conditions(
        &self,
    ) -> std::collections::HashMap<String, &FilterableField> {
        self.filterable_fields
            .iter()
            .filter(|f| f.condition.is_some())
            .map(|f| (f.name.clone(), f))
            .collect()
    }
}

const fn default_search_limit() -> u32 {
    DEFAULT_SEARCH_LIMIT
}

/// Type of a filterable payload field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterableFieldType {
    /// String/keyword field.
    Keyword,
    /// Integer field.
    Integer,
    /// Floating-point field.
    Float,
    /// Boolean field.
    Boolean,
}

/// Comparison condition for filterable fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum FilterableFieldCondition {
    /// Equal to.
    #[serde(rename = "==")]
    Equal,
    /// Not equal to.
    #[serde(rename = "!=")]
    NotEqual,
    /// Greater than.
    #[serde(rename = ">")]
    GreaterThan,
    /// Greater than or equal to.
    #[serde(rename = ">=")]
    GreaterThanOrEqual,
    /// Less than.
    #[serde(rename = "<")]
    LessThan,
    /// Less than or equal to.
    #[serde(rename = "<=")]
    LessThanOrEqual,
    /// Value is in the provided set.
    #[serde(rename = "any")]
    Any,
    /// Value is not in the provided set.
    #[serde(rename = "except")]
    Except,
}

/// Definition of a filterable payload field.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FilterableField {
    /// Name of the payload field to filter on.
    pub name: String,

    /// Human-readable description for the tool schema.
    pub description: String,

    /// Type of the field value.
    pub field_type: FilterableFieldType,

    /// Comparison condition. If `None`, the field is indexed but not exposed in the tool.
    #[serde(default)]
    pub condition: Option<FilterableFieldCondition>,

    /// Whether this filter is required in search queries.
    #[serde(default)]
    pub required: bool,
}

/// Combined settings for the Dear Diary MCP server.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Settings {
    /// Tool-related settings.
    #[serde(flatten)]
    pub tools: ToolSettings,

    /// Qdrant connection settings.
    #[serde(flatten)]
    pub qdrant: QdrantSettings,

    /// Embedding model name (e.g., `sentence-transformers/all-MiniLM-L6-v2`).
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
}

impl Settings {
    /// Loads settings from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if required configuration is missing or invalid.
    pub fn from_env() -> Result<Self, ConfigError> {
        let settings = Self {
            tools: ToolSettings {
                tool_store_description: std::env::var("TOOL_STORE_DESCRIPTION")
                    .unwrap_or_else(|_| DEFAULT_TOOL_STORE_DESCRIPTION.to_owned()),
                tool_find_description: std::env::var("TOOL_FIND_DESCRIPTION")
                    .unwrap_or_else(|_| DEFAULT_TOOL_FIND_DESCRIPTION.to_owned()),
            },
            qdrant: QdrantSettings {
                qdrant_url: std::env::var("QDRANT_URL").ok(),
                qdrant_api_key: std::env::var("QDRANT_API_KEY").ok(),
                collection_name: std::env::var("COLLECTION_NAME").ok(),
                qdrant_local_path: std::env::var("QDRANT_LOCAL_PATH").ok(),
                search_limit: std::env::var("QDRANT_SEARCH_LIMIT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(DEFAULT_SEARCH_LIMIT),
                read_only: env_var_bool("QDRANT_READ_ONLY"),
                filterable_fields: Vec::new(),
                allow_arbitrary_filter: env_var_bool("QDRANT_ALLOW_ARBITRARY_FILTER"),
            },
            embedding_model: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| DEFAULT_EMBEDDING_MODEL.to_owned()),
        };

        settings.qdrant.validate()?;
        Ok(settings)
    }
}

fn default_embedding_model() -> String {
    DEFAULT_EMBEDDING_MODEL.to_owned()
}

/// Parses a boolean value from an environment variable.
///
/// Returns `true` if the variable is set to "true" or "1", `false` otherwise.
fn env_var_bool(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "true" || v == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_default_tool_settings() {
        let settings = ToolSettings::default();
        assert_eq!(
            settings.tool_store_description,
            DEFAULT_TOOL_STORE_DESCRIPTION
        );
        assert_eq!(
            settings.tool_find_description,
            DEFAULT_TOOL_FIND_DESCRIPTION
        );
    }

    #[rstest]
    fn test_qdrant_settings_validate_both_set() {
        let settings = QdrantSettings {
            qdrant_url: Some("http://localhost:6334".to_owned()),
            qdrant_local_path: Some("/tmp/qdrant".to_owned()),
            ..Default::default()
        };
        assert!(matches!(
            settings.validate(),
            Err(ConfigError::ConflictingConnectionModes)
        ));
    }

    #[rstest]
    fn test_qdrant_settings_validate_neither_set() {
        let settings = QdrantSettings::default();
        assert!(matches!(
            settings.validate(),
            Err(ConfigError::MissingConnectionConfig)
        ));
    }

    #[rstest]
    fn test_qdrant_settings_validate_url_only() {
        let settings = QdrantSettings {
            qdrant_url: Some("http://localhost:6334".to_owned()),
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[rstest]
    fn test_qdrant_settings_validate_local_path_only() {
        let settings = QdrantSettings {
            qdrant_local_path: Some("/tmp/qdrant".to_owned()),
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[rstest]
    fn test_filterable_fields_map() {
        let settings = QdrantSettings {
            qdrant_url: Some("http://localhost:6334".to_owned()),
            filterable_fields: vec![
                FilterableField {
                    name: "category".to_owned(),
                    description: "Category filter".to_owned(),
                    field_type: FilterableFieldType::Keyword,
                    condition: Some(FilterableFieldCondition::Equal),
                    required: false,
                },
                FilterableField {
                    name: "priority".to_owned(),
                    description: "Priority filter".to_owned(),
                    field_type: FilterableFieldType::Integer,
                    condition: None,
                    required: false,
                },
            ],
            ..Default::default()
        };

        let map = settings.filterable_fields_map();
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("category"));
        assert!(map.contains_key("priority"));

        let with_conditions = settings.filterable_fields_with_conditions();
        assert_eq!(with_conditions.len(), 1);
        assert!(with_conditions.contains_key("category"));
    }
}
