//! Tests for `COLLECTION_NAME` interpolation and URL parsing.

use rstest::rstest;

use super::*;
use crate::error::ConfigError;
use parse::parse_remote_url;

// ── parse_remote_url ──────────────────────────────────────────

#[rstest]
#[case("https://github.com/leynos/dear-diary.git", "leynos", "dear-diary")]
#[case("https://github.com/leynos/dear-diary", "leynos", "dear-diary")]
#[case("git@github.com:leynos/dear-diary.git", "leynos", "dear-diary")]
#[case("git@github.com:leynos/dear-diary", "leynos", "dear-diary")]
#[case("https://gitlab.com/org/sub/repo.git", "org", "repo")]
#[case("git@gitlab.com:org/sub/repo.git", "org", "repo")]
#[case("https://codeberg.org/user/project.git", "user", "project")]
#[case("git@codeberg.org:user/project.git", "user", "project")]
#[case("git@git.sr.ht:~sircmpwn/aerc.git", "sircmpwn", "aerc")]
#[case("https://git.sr.ht/~sircmpwn/aerc", "sircmpwn", "aerc")]
#[case("git@bitbucket.org:team/repo.git", "team", "repo")]
#[case("https://bitbucket.org/team/repo.git", "team", "repo")]
fn test_parse_remote_url_extracts_owner_and_repo(
    #[case] url: &str,
    #[case] expected_owner: &str,
    #[case] expected_repo: &str,
) {
    let info = parse_remote_url(url).expect("parse_remote_url should succeed");
    assert_eq!(info.owner.as_deref(), Some(expected_owner));
    assert_eq!(info.repo, expected_repo);
}

// ── single-segment paths (repo only, no owner) ───────────────

#[rstest]
#[case("https://git.example.com/myrepo.git", "myrepo")]
#[case("https://git.example.com/myrepo", "myrepo")]
#[case("ssh://host/myrepo.git", "myrepo")]
fn test_parse_remote_url_single_segment_yields_repo(
    #[case] url: &str,
    #[case] expected_repo: &str,
) {
    let info = parse_remote_url(url).expect("parse_remote_url should succeed");
    assert!(
        info.owner.is_none(),
        "Single-segment path should have no owner"
    );
    assert_eq!(info.repo, expected_repo);
}

#[rstest]
fn test_parse_remote_url_error_on_empty_path() {
    // gix-url may normalise some bare-host URLs, so we construct
    // a pathological case that yields zero segments after stripping.
    let result = parse_remote_url("https://example.com/.git");
    assert!(
        result.is_err(),
        "Empty path after stripping should be rejected"
    );
}

// ── interpolate_collection_name ───────────────────────────────

#[rstest]
fn test_no_placeholders_is_passthrough() {
    let mock = MockGitContext::new();
    let result =
        interpolate_collection_name("plain-name", &mock).expect("interpolation should succeed");
    assert_eq!(result, "plain-name");
}

#[rstest]
fn test_repo_and_owner_resolved() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url()
        .returning(|| Ok(Some("git@github.com:leynos/dear-diary.git".to_owned())));

    let result = interpolate_collection_name("{owner}-{repo}-notes", &mock)
        .expect("interpolation should succeed");
    assert_eq!(result, "leynos-dear-diary-notes");
}

#[rstest]
fn test_cwd_resolved() {
    let mut mock = MockGitContext::new();
    mock.expect_cwd_basename()
        .returning(|| Ok("my-project".to_owned()));

    let result = interpolate_collection_name("{cwd}", &mock).expect("interpolation should succeed");
    assert_eq!(result, "my-project");
}

#[rstest]
fn test_branch_resolved() {
    let mut mock = MockGitContext::new();
    mock.expect_branch_name()
        .returning(|| Ok(Some("feature/cool-thing".to_owned())));

    let result =
        interpolate_collection_name("notes-{branch}", &mock).expect("interpolation should succeed");
    assert_eq!(result, "notes-feature/cool-thing");
}

#[rstest]
fn test_all_placeholders_combined() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url()
        .returning(|| Ok(Some("git@github.com:leynos/dear-diary.git".to_owned())));
    mock.expect_branch_name()
        .returning(|| Ok(Some("main".to_owned())));
    mock.expect_cwd_basename()
        .returning(|| Ok("workspace".to_owned()));

    let result = interpolate_collection_name("{owner}-{repo}-{cwd}-{branch}", &mock)
        .expect("interpolation should succeed");
    assert_eq!(result, "leynos-dear-diary-workspace-main");
}

#[rstest]
fn test_fails_when_no_remote_for_repo() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url().returning(|| Ok(None));

    let err = interpolate_collection_name("{repo}", &mock).expect_err("should fail without remote");
    assert!(
        matches!(err, ConfigError::UnresolvablePlaceholder { .. }),
        "Expected UnresolvablePlaceholder, got: {err:?}"
    );
}

#[rstest]
fn test_fails_when_no_remote_for_owner() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url().returning(|| Ok(None));

    let err =
        interpolate_collection_name("{owner}", &mock).expect_err("should fail without remote");
    assert!(
        matches!(err, ConfigError::UnresolvablePlaceholder { .. }),
        "Expected UnresolvablePlaceholder, got: {err:?}"
    );
}

#[rstest]
fn test_fails_when_no_remote_reports_both_placeholders() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url().returning(|| Ok(None));

    let err = interpolate_collection_name("{owner}-{repo}", &mock)
        .expect_err("should fail without remote");
    match err {
        ConfigError::UnresolvablePlaceholder {
            ref placeholder, ..
        } => {
            assert!(
                placeholder.contains("owner") && placeholder.contains("repo"),
                "Error should mention both placeholders, \
                 got: {placeholder}"
            );
        }
        _ => {
            panic!("Expected UnresolvablePlaceholder, got: {err:?}")
        }
    }
}

#[rstest]
fn test_fails_on_detached_head() {
    let mut mock = MockGitContext::new();
    mock.expect_branch_name().returning(|| Ok(None));

    let err =
        interpolate_collection_name("{branch}", &mock).expect_err("should fail on detached HEAD");
    assert!(
        matches!(err, ConfigError::UnresolvablePlaceholder { .. }),
        "Expected UnresolvablePlaceholder, got: {err:?}"
    );
}

#[rstest]
fn test_source_hut_tilde_stripped() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url()
        .returning(|| Ok(Some("git@git.sr.ht:~sircmpwn/aerc.git".to_owned())));

    let result =
        interpolate_collection_name("{owner}", &mock).expect("interpolation should succeed");
    assert_eq!(result, "sircmpwn");
}

#[rstest]
fn test_cwd_does_not_require_git() {
    let mut mock = MockGitContext::new();
    mock.expect_cwd_basename()
        .returning(|| Ok("standalone-dir".to_owned()));

    let result =
        interpolate_collection_name("notes-{cwd}", &mock).expect("interpolation should succeed");
    assert_eq!(result, "notes-standalone-dir");
}

#[rstest]
fn test_unknown_braces_left_as_is() {
    let mock = MockGitContext::new();
    let result =
        interpolate_collection_name("{foo}-literal", &mock).expect("interpolation should succeed");
    assert_eq!(result, "{foo}-literal");
}

#[rstest]
fn test_git_command_error_propagated_from_remote() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url()
        .returning(|| Err(ConfigError::GitCommandError("permission denied".to_owned())));

    let err = interpolate_collection_name("{repo}", &mock).expect_err("should propagate git error");
    assert!(
        matches!(err, ConfigError::GitCommandError(..)),
        "Expected GitCommandError, got: {err:?}"
    );
}

#[rstest]
fn test_git_command_error_propagated_from_branch() {
    let mut mock = MockGitContext::new();
    mock.expect_branch_name()
        .returning(|| Err(ConfigError::GitCommandError("permission denied".to_owned())));

    let err =
        interpolate_collection_name("{branch}", &mock).expect_err("should propagate git error");
    assert!(
        matches!(err, ConfigError::GitCommandError(..)),
        "Expected GitCommandError, got: {err:?}"
    );
}

// ── single-segment remote + interpolation ─────────────────────

#[rstest]
fn test_repo_resolves_from_single_segment_remote() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url()
        .returning(|| Ok(Some("https://git.example.com/myrepo.git".to_owned())));

    let result = interpolate_collection_name("{repo}", &mock)
        .expect("should resolve repo from single-segment URL");
    assert_eq!(result, "myrepo");
}

#[rstest]
fn test_owner_fails_on_single_segment_remote() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url()
        .returning(|| Ok(Some("https://git.example.com/myrepo.git".to_owned())));

    let err = interpolate_collection_name("{owner}", &mock)
        .expect_err("should fail for owner with single-segment URL");
    assert!(
        matches!(err, ConfigError::UnresolvablePlaceholder { .. }),
        "Expected UnresolvablePlaceholder, got: {err:?}"
    );
}

#[rstest]
fn test_owner_and_repo_fails_on_single_segment_remote() {
    let mut mock = MockGitContext::new();
    mock.expect_remote_url()
        .returning(|| Ok(Some("https://git.example.com/myrepo.git".to_owned())));

    let err = interpolate_collection_name("{owner}-{repo}", &mock)
        .expect_err("should fail for owner with single-segment URL");
    assert!(
        matches!(err, ConfigError::UnresolvablePlaceholder { .. }),
        "Expected UnresolvablePlaceholder, got: {err:?}"
    );
}
