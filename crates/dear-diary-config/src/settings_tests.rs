//! Unit tests for configuration defaults and validation rules.

use super::*;
use crate::interpolation::MockGitContext;
use ortho_config::{EnvSource, MapEnv};
use rstest::rstest;

/// A variable lookup over the closed `env`, as `Settings::from_lookup_with_git`
/// takes it.
fn lookup_in(env: &MapEnv) -> impl Fn(&str) -> Option<String> + '_ {
    |name| env.get(name).and_then(|value| value.into_string().ok())
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
#[case(Some("http://localhost:6334".to_owned()), None)]
#[case(None, Some("/tmp/qdrant".to_owned()))]
fn test_qdrant_settings_validate_single_connection_mode(
    #[case] qdrant_url: Option<String>,
    #[case] qdrant_local_path: Option<String>,
) {
    let settings = QdrantSettings {
        qdrant_url,
        qdrant_local_path,
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
    let env = MapEnv::new()
        .with_var("QDRANT_URL", "http://localhost:6334")
        .with_var("COLLECTION_NAME", "{owner}-{repo}-{cwd}-{branch}");

    let mut git = MockGitContext::new();
    git.expect_remote_url()
        .returning(|| Ok(Some("git@github.com:leynos/dear-diary.git".to_owned())));
    git.expect_cwd_basename()
        .returning(|| Ok("workspace".to_owned()));
    git.expect_branch_name()
        .returning(|| Ok(Some("adopt-whitaker-lints".to_owned())));

    let settings = Settings::from_lookup_with_git(lookup_in(&env), &git)
        .expect("settings should load from environment");

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
    let env = MapEnv::new()
        .with_var("QDRANT_URL", "http://localhost:6334")
        .with_var("COLLECTION_NAME", "{owner}-{repo}");

    let mut git = MockGitContext::new();
    git.expect_remote_url().returning(|| Ok(None));

    let err = Settings::from_lookup_with_git(lookup_in(&env), &git)
        .expect_err("settings should reject unresolved collection placeholders");

    assert!(
        matches!(err, ConfigError::UnresolvablePlaceholder { .. }),
        "expected unresolved placeholder error, got {err:?}"
    );
}
