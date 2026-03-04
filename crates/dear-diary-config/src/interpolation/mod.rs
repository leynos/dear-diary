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

mod parse;

#[cfg(test)]
mod tests;

use crate::error::ConfigError;
use parse::parse_remote_url;

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
                    "Failed to run \
                     'git remote get-url origin': {e}"
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
                    "Failed to run \
                     'git rev-parse --abbrev-ref HEAD': {e}"
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
                    "Current directory has no basename \
                     (e.g. root path)"
                        .to_owned(),
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
            let affected = unresolved_remote_placeholders(needs_owner, needs_repo);
            ConfigError::UnresolvablePlaceholder {
                placeholder: affected,
                reason: "No git remote named 'origin' is \
                         configured"
                    .to_owned(),
            }
        })?;

        let info = parse_remote_url(&url)?;

        if needs_owner {
            let owner = info
                .owner
                .ok_or_else(|| ConfigError::UnresolvablePlaceholder {
                    placeholder: "owner".to_owned(),
                    reason: format!(
                        concat!(
                            "Remote URL '{0}' has a ",
                            "single-segment path with ",
                            "no owner component"
                        ),
                        url
                    ),
                })?;
            result = result.replace("{owner}", &owner);
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
        // Structurally unreachable: called only when at least one
        // is true.
        (false, false) => String::new(),
    }
}
