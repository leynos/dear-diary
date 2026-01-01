//! Entry data model for stored memories.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Metadata attached to an entry.
pub type Metadata = HashMap<String, Value>;

/// A single entry stored in the Qdrant collection.
///
/// Each entry consists of textual content and optional JSON metadata.
/// The content is embedded and stored as a vector for semantic search,
/// while metadata can be used for filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// The textual content of the entry.
    pub content: String,

    /// Optional metadata associated with the entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl Entry {
    /// Creates a new entry with the given content and no metadata.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            metadata: None,
        }
    }

    /// Creates a new entry with content and metadata.
    #[must_use]
    pub fn with_metadata(content: impl Into<String>, metadata: Metadata) -> Self {
        Self {
            content: content.into(),
            metadata: Some(metadata),
        }
    }
}

/// A search result containing an entry with its Qdrant point ID and deprecation state.
///
/// This struct is returned from search operations and includes additional
/// information needed for subsequent operations like deprecation.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The unique Qdrant point ID for this entry.
    pub point_id: String,

    /// The entry content and metadata.
    pub entry: Entry,

    /// Unix timestamp when this entry was deprecated, if applicable.
    /// `None` means the entry is active (not deprecated).
    pub deprecated_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    #[rstest]
    fn test_entry_new() {
        let entry = Entry::new("Hello, world!");
        assert_eq!(entry.content, "Hello, world!");
        assert!(entry.metadata.is_none());
    }

    #[rstest]
    fn test_entry_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_owned(), json!("value"));

        let entry = Entry::with_metadata("Hello", metadata);
        assert_eq!(entry.content, "Hello");
        assert!(entry.metadata.is_some());
        assert_eq!(
            entry.metadata.as_ref().and_then(|m| m.get("key")),
            Some(&json!("value"))
        );
    }

    #[rstest]
    fn test_entry_serialization() {
        let entry = Entry::new("Test content");
        let json = serde_json::to_string(&entry).ok();
        assert!(json.is_some());
        assert!(json.as_ref().is_some_and(|j| j.contains("Test content")));
    }
}
