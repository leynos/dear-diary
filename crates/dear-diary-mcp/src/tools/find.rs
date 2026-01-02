//! Find tool request type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request parameters for the qdrant-find tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FindRequest {
    /// The search query to find relevant information.
    #[schemars(description = "The search query to find relevant information")]
    pub query: String,

    /// The name of the collection to search in.
    /// If not provided, the default collection will be used.
    #[schemars(
        description = "The name of the collection to search in (optional if default is configured)"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_name: Option<String>,

    /// Reserved for future filter support.
    ///
    /// **Note**: Arbitrary filter parsing is not yet implemented.
    #[schemars(description = "Reserved for future filter support (not yet implemented)")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Value>,

    /// Include deprecated entries in search results.
    /// By default, entries deprecated more than 7 days ago are hidden.
    /// Set to true to include all deprecated entries.
    #[schemars(
        description = "Include deprecated entries that would normally be hidden (older than 7 days)"
    )]
    #[serde(default)]
    pub include_deprecated: bool,
}
