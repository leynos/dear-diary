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

/// Loads settings from `env` with a git context that no case here consults.
fn load(env: &MapEnv) -> Result<Settings, ConfigError> {
    Settings::from_lookup_with_git(lookup_in(env), &MockGitContext::new())
}

/// The smallest environment that passes validation.
fn connected_env() -> MapEnv {
    MapEnv::new().with_var("QDRANT_URL", "http://localhost:6334")
}

#[rstest]
fn lookup_defaults_apply_when_only_a_connection_is_named() {
    let settings = load(&connected_env()).expect("a connected environment loads");

    assert_eq!(settings.embedding_model, DEFAULT_EMBEDDING_MODEL);
    assert_eq!(
        settings.tools.tool_store_description,
        DEFAULT_TOOL_STORE_DESCRIPTION
    );
    assert_eq!(
        settings.tools.tool_find_description,
        DEFAULT_TOOL_FIND_DESCRIPTION
    );
    assert_eq!(settings.qdrant.search_limit, DEFAULT_SEARCH_LIMIT);
    assert!(!settings.qdrant.read_only);
    assert!(!settings.qdrant.allow_arbitrary_filter);
    assert_eq!(settings.qdrant.qdrant_api_key, None);
    assert_eq!(settings.qdrant.collection_name, None);
}

#[rstest]
fn lookup_overrides_replace_every_default() {
    let env = connected_env()
        .with_var("QDRANT_API_KEY", "secret")
        .with_var("COLLECTION_NAME", "plain-name")
        .with_var("EMBEDDING_MODEL", "custom/model")
        .with_var("QDRANT_SEARCH_LIMIT", "25")
        .with_var("QDRANT_READ_ONLY", "true")
        .with_var("QDRANT_ALLOW_ARBITRARY_FILTER", "1")
        .with_var("TOOL_STORE_DESCRIPTION", "store it")
        .with_var("TOOL_FIND_DESCRIPTION", "find it");

    let mut git = MockGitContext::new();
    git.expect_remote_url().returning(|| Ok(None));
    let settings = Settings::from_lookup_with_git(lookup_in(&env), &git)
        .expect("a fully named environment loads");

    assert_eq!(settings.embedding_model, "custom/model");
    assert_eq!(settings.tools.tool_store_description, "store it");
    assert_eq!(settings.tools.tool_find_description, "find it");
    assert_eq!(settings.qdrant.qdrant_api_key.as_deref(), Some("secret"));
    assert_eq!(
        settings.qdrant.collection_name.as_deref(),
        Some("plain-name")
    );
    assert_eq!(settings.qdrant.search_limit, 25);
    assert!(settings.qdrant.read_only);
    assert!(settings.qdrant.allow_arbitrary_filter);
}

#[rstest]
#[case::true_word("true", true)]
#[case::one("1", true)]
#[case::false_word("false", false)]
#[case::zero("0", false)]
#[case::capitalised("True", false)]
#[case::yes("yes", false)]
#[case::empty("", false)]
fn lookup_parses_boolean_flags(#[case] value: &str, #[case] expected: bool) {
    let env = connected_env()
        .with_var("QDRANT_READ_ONLY", value)
        .with_var("QDRANT_ALLOW_ARBITRARY_FILTER", value);

    let settings = load(&env).expect("a connected environment loads");

    assert_eq!(settings.qdrant.read_only, expected);
    assert_eq!(settings.qdrant.allow_arbitrary_filter, expected);
}

#[rstest]
#[case::valid("25", 25)]
#[case::zero("0", 0)]
#[case::not_a_number("many", DEFAULT_SEARCH_LIMIT)]
#[case::negative("-3", DEFAULT_SEARCH_LIMIT)]
#[case::empty("", DEFAULT_SEARCH_LIMIT)]
fn lookup_parses_the_search_limit(#[case] value: &str, #[case] expected: u32) {
    let env = connected_env().with_var("QDRANT_SEARCH_LIMIT", value);

    let settings = load(&env).expect("a connected environment loads");

    assert_eq!(settings.qdrant.search_limit, expected);
}

#[rstest]
fn lookup_rejects_both_connection_modes() {
    let env = connected_env().with_var("QDRANT_LOCAL_PATH", "/tmp/qdrant");

    assert!(matches!(
        load(&env),
        Err(ConfigError::ConflictingConnectionModes)
    ));
}

#[rstest]
fn lookup_rejects_a_missing_connection() {
    assert!(matches!(
        load(&MapEnv::new()),
        Err(ConfigError::MissingConnectionConfig)
    ));
}

/// A value that is not valid Unicode reads as absent, as `std::env::var(..).ok()`
/// did, so the other connection mode still validates on its own.
#[cfg(unix)]
#[rstest]
fn lookup_treats_a_non_unicode_value_as_absent() {
    use std::os::unix::ffi::OsStringExt as _;

    let env = MapEnv::new()
        .with_var("QDRANT_URL", std::ffi::OsString::from_vec(vec![0xff, 0xfe]))
        .with_var("QDRANT_LOCAL_PATH", "/tmp/qdrant");

    let settings = load(&env).expect("the non-Unicode URL reads as absent");

    assert_eq!(settings.qdrant.qdrant_url, None);
    assert_eq!(
        settings.qdrant.qdrant_local_path.as_deref(),
        Some("/tmp/qdrant")
    );
}

/// Marks the re-run of this binary that reads its own live environment.
const CHILD_MARKER: &str = "DEAR_DIARY_TEST_PROCESS_PATH_CHILD";

/// `Settings::from_env_with_git` reads the live process environment on purpose,
/// so the parent re-runs this binary as a child that carries the variables and
/// changes nothing in its own process. The child inherits the rest of the
/// parent's environment, which keeps `LLVM_PROFILE_FILE` for coverage runs.
#[rstest]
fn from_env_with_git_reads_the_process_environment() {
    if std::env::var_os(CHILD_MARKER).is_some() {
        assert_process_path_reads_the_environment();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().expect("test binary path"));
    child
        .args([
            "--exact",
            "settings::tests::from_env_with_git_reads_the_process_environment",
            "--nocapture",
        ])
        .env(CHILD_MARKER, "1")
        .env("QDRANT_URL", "http://localhost:6334")
        .env("COLLECTION_NAME", "{owner}-{repo}")
        .env("QDRANT_SEARCH_LIMIT", "42");
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt as _;
        child.env(
            "QDRANT_API_KEY",
            std::ffi::OsString::from_vec(vec![0xff, 0xfe]),
        );
    }
    let status = child.status().expect("run the child test binary");
    assert!(status.success(), "the child run failed: {status}");
}

/// The child half: the production path sees the variables, resolves the
/// placeholders through the git context, and reads a non-Unicode value as
/// absent.
fn assert_process_path_reads_the_environment() {
    let mut git = MockGitContext::new();
    git.expect_remote_url()
        .returning(|| Ok(Some("git@github.com:leynos/dear-diary.git".to_owned())));

    let settings = Settings::from_env_with_git(&git).expect("the child environment loads");

    assert_eq!(
        settings.qdrant.qdrant_url.as_deref(),
        Some("http://localhost:6334")
    );
    assert_eq!(
        settings.qdrant.collection_name.as_deref(),
        Some("leynos-dear-diary")
    );
    assert_eq!(settings.qdrant.search_limit, 42);
    #[cfg(unix)]
    assert_eq!(settings.qdrant.qdrant_api_key, None);
}
