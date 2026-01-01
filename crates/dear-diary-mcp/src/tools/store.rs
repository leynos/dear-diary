//! Store tool request type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Request parameters for the qdrant-store tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StoreRequest {
    /// The information to store in the database.
    #[schemars(description = "The information to store in the database")]
    pub information: String,

    /// Optional metadata to attach to the stored information.
    #[schemars(description = "Optional metadata to attach to the stored information")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,

    /// The name of the collection to store the information in.
    /// If not provided, the default collection will be used.
    #[schemars(
        description = "The name of the collection to store in (optional if default is configured)"
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_name: Option<String>,
}
