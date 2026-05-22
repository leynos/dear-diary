//! Unit tests for configuration defaults and validation rules.

use super::*;
use crate::interpolation::MockGitContext;
use rstest::rstest;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

const SETTINGS_ENV_KEYS: &[&str] = &[
    "COLLECTION_NAME",
    "EMBEDDING_MODEL",
    "QDRANT_ALLOW_ARBITRARY_FILTER",
    "QDRANT_API_KEY",
    "QDRANT_LOCAL_PATH",
    "QDRANT_READ_ONLY",
    "QDRANT_SEARCH_LIMIT",
    "QDRANT_URL",
    "TOOL_FIND_DESCRIPTION",
    "TOOL_STORE_DESCRIPTION",
];

/// Guard that restores settings-related environment variables on drop.
struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    saved: HashMap<&'static str, Option<String>>,
}

impl EnvGuard {
    /// Captures and clears settings-related environment variables.
    fn new() -> Self {
        let lock = match ENV_MUTEX.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        let saved = SETTINGS_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect();
        let guard = Self { _lock: lock, saved };
        for key in SETTINGS_ENV_KEYS {
            Self::remove(key);
        }
        guard
    }

    /// Sets an environment variable while the guard serializes access.
    fn set(key: &'static str, value: &str) {
        // SAFETY: tests mutate process-global environment only while holding
        // ENV_MUTEX, and EnvGuard restores the previous values.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    /// Removes an environment variable while the guard serializes access.
    fn remove(key: &'static str) {
        // SAFETY: tests mutate process-global environment only while holding
        // ENV_MUTEX, and EnvGuard restores the previous values.
        unsafe {
            std::env::remove_var(key);
        }
    }

    /// Restores one environment variable to its captured value.
    fn restore(key: &'static str, saved_value: Option<&str>) {
        match saved_value {
            Some(original) => Self::set(key, original),
            None => Self::remove(key),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, saved_value) in &self.saved {
            Self::restore(key, saved_value.as_deref());
        }
    }
}

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

#[rstest]
fn test_from_env_with_git_resolves_collection_name_placeholders() {
    let _env = EnvGuard::new();
    EnvGuard::set("QDRANT_URL", "http://localhost:6334");
    EnvGuard::set("COLLECTION_NAME", "{owner}-{repo}-{cwd}-{branch}");

    let mut git = MockGitContext::new();
    git.expect_remote_url()
        .returning(|| Ok(Some("git@github.com:leynos/dear-diary.git".to_owned())));
    git.expect_cwd_basename()
        .returning(|| Ok("workspace".to_owned()));
    git.expect_branch_name()
        .returning(|| Ok(Some("adopt-whitaker-lints".to_owned())));

    let settings =
        Settings::from_env_with_git(&git).expect("settings should load from environment");

    assert_eq!(
        settings.qdrant.collection_name.as_deref(),
        Some("leynos-dear-diary-workspace-adopt-whitaker-lints")
    );
    assert_eq!(
        settings.qdrant.qdrant_url.as_deref(),
        Some("http://localhost:6334")
    );
}

#[rstest]
fn test_from_env_with_git_reports_unresolved_collection_name_placeholders() {
    let _env = EnvGuard::new();
    EnvGuard::set("QDRANT_URL", "http://localhost:6334");
    EnvGuard::set("COLLECTION_NAME", "{owner}-{repo}");

    let mut git = MockGitContext::new();
    git.expect_remote_url().returning(|| Ok(None));

    let err = Settings::from_env_with_git(&git)
        .expect_err("settings should reject unresolved collection placeholders");

    assert!(
        matches!(err, ConfigError::UnresolvablePlaceholder { .. }),
        "expected unresolved placeholder error, got {err:?}"
    );
}
