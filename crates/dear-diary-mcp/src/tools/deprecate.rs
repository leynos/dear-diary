//! Deprecate tool request type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Request parameters for the qdrant-deprecate tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeprecateRequest {
    /// The search query to find the entry to deprecate.
    /// The top matching result will be marked as deprecated.
    #[schemars(description = "Search query to find the entry to deprecate (top match will be deprecated)")]
    pub query: String,

    /// The name of the collection containing the entry.
    /// If not provided, the default collection will be used.
    #[schemars(description = "The name of the collection (optional if default is configured)")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_name: Option<String>,
}
