//! Interpolation support for `COLLECTION_NAME` placeholders.
//!
//! This module resolves placeholders in the `COLLECTION_NAME` environment
//! variable at startup, substituting values from the current git repository
//! and working directory.
//!
//! Supported placeholders:
//!
//! | Placeholder  | Source                                |
//! |--------------|---------------------------------------|
//! | `{repo}`     | Repository name from `origin` remote  |
//! | `{owner}`    | Repository owner from `origin` remote |
//! | `{cwd}`      | Basename of current working directory |
//! | `{branch}`   | Current git branch name               |

use bstr::ByteSlice;

use crate::error::ConfigError;

/// Abstraction over git and working-directory queries for testability.
///
/// Production code uses [`RealGitContext`]; tests substitute a mock.
#[cfg_attr(test, mockall::automock)]
pub trait GitContext {
    /// Returns the URL of the git remote named "origin", if available.
    ///
    /// Returns `Ok(None)` when no remote named "origin" is configured.
    ///
    /// # Errors
    ///
    /// Returns an error if the git command could not be executed or
    /// failed for an unexpected reason.
    fn remote_url(&self) -> Result<Option<String>, ConfigError>;

    /// Returns the current git branch name, if available.
    ///
    /// Returns `Ok(None)` when HEAD is detached.
    ///
    /// # Errors
    ///
    /// Returns an error if the git command could not be executed or
    /// failed for an unexpected reason.
    fn branch_name(&self) -> Result<Option<String>, ConfigError>;

    /// Returns the basename of the current working directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory cannot be determined.
    fn cwd_basename(&self) -> Result<String, ConfigError>;
}

/// Production implementation of [`GitContext`] using git CLI commands.
pub struct RealGitContext;

impl GitContext for RealGitContext {
    fn remote_url(&self) -> Result<Option<String>, ConfigError> {
        let output = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .map_err(|e| {
                ConfigError::GitCommandError(format!(
                    "Failed to run 'git remote get-url origin': {e}"
                ))
            })?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            return Ok(if url.is_empty() { None } else { Some(url) });
        }

        // Distinguish "no such remote" from genuine failures.
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No such remote") {
            return Ok(None);
        }

        Err(ConfigError::GitCommandError(format_git_failure(
            "git remote get-url origin",
            output.status,
            &stderr,
        )))
    }

    fn branch_name(&self) -> Result<Option<String>, ConfigError> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map_err(|e| {
                ConfigError::GitCommandError(format!(
                    "Failed to run 'git rev-parse --abbrev-ref HEAD': {e}"
                ))
            })?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            // Detached HEAD returns the literal string "HEAD".
            return Ok(if branch.is_empty() || branch == "HEAD" {
                None
            } else {
                Some(branch)
            });
        }

        // Distinguish "not a git repo" from genuine failures.
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not a git repository") {
            return Ok(None);
        }

        Err(ConfigError::GitCommandError(format_git_failure(
            "git rev-parse --abbrev-ref HEAD",
            output.status,
            &stderr,
        )))
    }

    fn cwd_basename(&self) -> Result<String, ConfigError> {
        let cwd = std::env::current_dir().map_err(|e| {
            ConfigError::InterpolationContextError(format!(
                "Failed to determine current directory: {e}"
            ))
        })?;
        cwd.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| {
                ConfigError::InterpolationContextError(
                    "Current directory has no basename (e.g. root path)".to_owned(),
                )
            })
    }
}

/// Formats a human-readable error message for a failed git command.
fn format_git_failure(command: &str, status: std::process::ExitStatus, stderr: &str) -> String {
    let code = status
        .code()
        .map_or_else(|| "unknown".to_owned(), |c| c.to_string());
    format!("{command} failed (exit code {code}): {}", stderr.trim())
}

/// Parsed owner and repository name from a git remote URL.
#[derive(Debug)]
struct RemoteInfo {
    owner: String,
    repo: String,
}

/// Extracts the owner and repository name from a git remote URL.
///
/// Supports HTTPS, SSH, and SCP-style URLs. Strips the `.git` suffix
/// from the repository name. For Source Hut URLs, the tilde prefix is
/// stripped from the owner (e.g. `~user` becomes `user`).
///
/// For GitLab subgroup URLs (e.g. `org/sub/subsub/repo`), the owner
/// is the first path segment and the repo is the last.
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or does not contain
/// recognisable owner/repo path segments.
fn parse_remote_url(raw_url: &str) -> Result<RemoteInfo, ConfigError> {
    let url = gix_url::parse(raw_url.into()).map_err(|e| {
        ConfigError::InterpolationContextError(format!(
            "Failed to parse git remote URL '{raw_url}': {e}"
        ))
    })?;

    let path_str = url.path.to_str().map_err(|e| {
        ConfigError::InterpolationContextError(format!(
            "Git remote URL path is not valid UTF-8: {e}"
        ))
    })?;

    // Strip leading '/' and trailing '.git'.
    let without_prefix = path_str.strip_prefix('/').unwrap_or(path_str);
    let clean_path = without_prefix
        .strip_suffix(".git")
        .unwrap_or(without_prefix);

    // Walk segments once, tracking the first and last non-empty values.
    let mut first_segment: Option<&str> = None;
    let mut last_segment: Option<&str> = None;
    let mut segment_count: usize = 0;

    for segment in clean_path.split('/').filter(|s| !s.is_empty()) {
        if first_segment.is_none() {
            first_segment = Some(segment);
        }
        last_segment = Some(segment);
        segment_count += 1;
    }

    if segment_count < 2 {
        return Err(ConfigError::InterpolationContextError(format!(
            concat!(
                "Cannot extract owner/repo from remote URL '{0}': ",
                "path does not contain owner/repo segments"
            ),
            raw_url
        )));
    }

    // SAFETY (logic): segment_count >= 2 guarantees both are Some.
    let owner_raw = first_segment.ok_or_else(|| {
        ConfigError::InterpolationContextError(format!(
            "Cannot extract owner/repo from remote URL '{raw_url}'"
        ))
    })?;
    let repo = last_segment.ok_or_else(|| {
        ConfigError::InterpolationContextError(format!(
            "Cannot extract owner/repo from remote URL '{raw_url}'"
        ))
    })?;

    // Strip Source Hut tilde prefix from owner.
    let owner = owner_raw.strip_prefix('~').unwrap_or(owner_raw);

    Ok(RemoteInfo {
        owner: owner.to_owned(),
        repo: repo.to_owned(),
    })
}

/// Interpolates placeholders in a collection name template.
///
/// Only placeholders that appear in `template` trigger resolution.
/// Unknown text resembling a placeholder (e.g. `{foo}`) is left as-is.
///
/// # Errors
///
/// Returns an error if a placeholder is present but the corresponding
/// value cannot be determined (e.g. no `origin` remote for `{repo}`).
pub fn interpolate_collection_name(
    template: &str,
    git: &impl GitContext,
) -> Result<String, ConfigError> {
    let needs_repo = template.contains("{repo}");
    let needs_owner = template.contains("{owner}");
    let needs_cwd = template.contains("{cwd}");
    let needs_branch = template.contains("{branch}");

    // Fast path: no placeholders at all.
    if !needs_repo && !needs_owner && !needs_cwd && !needs_branch {
        return Ok(template.to_owned());
    }

    let mut result = template.to_owned();

    // Resolve remote-derived placeholders together (one git call).
    if needs_repo || needs_owner {
        let url = git.remote_url()?.ok_or_else(|| {
            // Report all affected placeholders so the user sees the full picture.
            let affected = unresolved_remote_placeholders(needs_owner, needs_repo);
            ConfigError::UnresolvablePlaceholder {
                placeholder: affected,
                reason: "No git remote named 'origin' is configured".to_owned(),
            }
        })?;

        let info = parse_remote_url(&url)?;

        if needs_owner {
            result = result.replace("{owner}", &info.owner);
        }
        if needs_repo {
            result = result.replace("{repo}", &info.repo);
        }
    }

    if needs_cwd {
        let basename = git.cwd_basename()?;
        result = result.replace("{cwd}", &basename);
    }

    if needs_branch {
        let branch = git
            .branch_name()?
            .ok_or_else(|| ConfigError::UnresolvablePlaceholder {
                placeholder: "branch".to_owned(),
                reason: concat!(
                    "Not on a named branch ",
                    "(detached HEAD or not a git repository)"
                )
                .to_owned(),
            })?;
        result = result.replace("{branch}", &branch);
    }

    Ok(result)
}

/// Builds a comma-separated list of remote-derived placeholders that
/// could not be resolved, for inclusion in error messages.
fn unresolved_remote_placeholders(needs_owner: bool, needs_repo: bool) -> String {
    match (needs_owner, needs_repo) {
        (true, true) => "owner, repo".to_owned(),
        (true, false) => "owner".to_owned(),
        (false, true) => "repo".to_owned(),
        // Structurally unreachable: called only when at least one is true.
        (false, false) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

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
        assert_eq!(info.owner, expected_owner);
        assert_eq!(info.repo, expected_repo);
    }

    #[rstest]
    fn test_parse_remote_url_rejects_single_segment() {
        let result = parse_remote_url("https://example.com/solo");
        assert!(result.is_err(), "Single path segment should be rejected");
    }

    #[rstest]
    fn test_parse_remote_url_error_is_interpolation_context() {
        let err = parse_remote_url("https://example.com/solo")
            .expect_err("should fail with single segment");
        assert!(
            matches!(err, ConfigError::InterpolationContextError(..)),
            "Expected InterpolationContextError, got: {err:?}"
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

        let result =
            interpolate_collection_name("{cwd}", &mock).expect("interpolation should succeed");
        assert_eq!(result, "my-project");
    }

    #[rstest]
    fn test_branch_resolved() {
        let mut mock = MockGitContext::new();
        mock.expect_branch_name()
            .returning(|| Ok(Some("feature/cool-thing".to_owned())));

        let result = interpolate_collection_name("notes-{branch}", &mock)
            .expect("interpolation should succeed");
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

        let err =
            interpolate_collection_name("{repo}", &mock).expect_err("should fail without remote");
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
                    "Error should mention both placeholders, got: {placeholder}"
                );
            }
            _ => panic!("Expected UnresolvablePlaceholder, got: {err:?}"),
        }
    }

    #[rstest]
    fn test_fails_on_detached_head() {
        let mut mock = MockGitContext::new();
        mock.expect_branch_name().returning(|| Ok(None));

        let err = interpolate_collection_name("{branch}", &mock)
            .expect_err("should fail on detached HEAD");
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
        // Only cwd_basename is called; remote_url and branch_name are not.
        mock.expect_cwd_basename()
            .returning(|| Ok("standalone-dir".to_owned()));

        let result = interpolate_collection_name("notes-{cwd}", &mock)
            .expect("interpolation should succeed");
        assert_eq!(result, "notes-standalone-dir");
    }

    #[rstest]
    fn test_unknown_braces_left_as_is() {
        let mock = MockGitContext::new();
        let result = interpolate_collection_name("{foo}-literal", &mock)
            .expect("interpolation should succeed");
        assert_eq!(result, "{foo}-literal");
    }

    #[rstest]
    fn test_git_command_error_propagated_from_remote() {
        let mut mock = MockGitContext::new();
        mock.expect_remote_url()
            .returning(|| Err(ConfigError::GitCommandError("permission denied".to_owned())));

        let err =
            interpolate_collection_name("{repo}", &mock).expect_err("should propagate git error");
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
}
