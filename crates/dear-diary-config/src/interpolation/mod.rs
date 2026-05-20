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

/// Tracks which interpolation placeholders appear in a template.
struct PlaceholderNeeds(u8);

impl PlaceholderNeeds {
    const REPO: u8 = 1 << 0;
    const OWNER: u8 = 1 << 1;
    const CWD: u8 = 1 << 2;
    const BRANCH: u8 = 1 << 3;

    /// Builds placeholder requirements by scanning the template once per token.
    fn from_template(template: &str) -> Self {
        let mut flags = 0;
        flags |= placeholder_flag(template, "{repo}", Self::REPO);
        flags |= placeholder_flag(template, "{owner}", Self::OWNER);
        flags |= placeholder_flag(template, "{cwd}", Self::CWD);
        flags |= placeholder_flag(template, "{branch}", Self::BRANCH);
        Self(flags)
    }

    /// Returns true when the template contains no supported placeholders.
    const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Returns true when interpolation must inspect the git remote.
    const fn needs_remote(&self) -> bool {
        self.has(Self::REPO | Self::OWNER)
    }

    /// Returns true when the repository placeholder appears in the template.
    const fn needs_repo(&self) -> bool {
        self.has(Self::REPO)
    }

    /// Returns true when the owner placeholder appears in the template.
    const fn needs_owner(&self) -> bool {
        self.has(Self::OWNER)
    }

    /// Returns true when the working-directory placeholder appears.
    const fn needs_cwd(&self) -> bool {
        self.has(Self::CWD)
    }

    /// Returns true when the branch placeholder appears in the template.
    const fn needs_branch(&self) -> bool {
        self.has(Self::BRANCH)
    }

    /// Returns true when any selected placeholder flag is set.
    const fn has(&self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

/// Returns the flag when the template contains a placeholder.
fn placeholder_flag(template: &str, placeholder: &str, flag: u8) -> u8 {
    if template.contains(placeholder) {
        flag
    } else {
        0
    }
}

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
    let needs = PlaceholderNeeds::from_template(template);

    // Fast path: no placeholders at all.
    if needs.is_empty() {
        return Ok(template.to_owned());
    }

    let mut result = template.to_owned();

    if needs.needs_remote() {
        result = replace_remote_placeholders(result, git, &needs)?;
    }

    result = replace_cwd_placeholder(result, git, &needs)?;
    result = replace_branch_placeholder(result, git, &needs)?;

    Ok(result)
}

/// Replaces placeholders derived from the `origin` remote.
fn replace_remote_placeholders(
    mut result: String,
    git: &impl GitContext,
    needs: &PlaceholderNeeds,
) -> Result<String, ConfigError> {
    let url = git.remote_url()?.ok_or_else(|| {
        let affected = unresolved_remote_placeholders(needs.needs_owner(), needs.needs_repo());
        ConfigError::UnresolvablePlaceholder {
            placeholder: affected,
            reason: "No git remote named 'origin' is \
                     configured"
                .to_owned(),
        }
    })?;

    let info = parse_remote_url(&url)?;

    if needs.needs_owner() {
        let owner = remote_owner(&url, info.owner)?;
        result = result.replace("{owner}", &owner);
    }

    if needs.needs_repo() {
        result = result.replace("{repo}", &info.repo);
    }

    Ok(result)
}

/// Returns a remote owner or the domain error for single-segment remotes.
fn remote_owner(url: &str, owner: Option<String>) -> Result<String, ConfigError> {
    owner.ok_or_else(|| ConfigError::UnresolvablePlaceholder {
        placeholder: "owner".to_owned(),
        reason: format!(
            concat!(
                "Remote URL '{0}' has a ",
                "single-segment path with ",
                "no owner component"
            ),
            url
        ),
    })
}

/// Replaces the working-directory placeholder when present.
fn replace_cwd_placeholder(
    result: String,
    git: &impl GitContext,
    needs: &PlaceholderNeeds,
) -> Result<String, ConfigError> {
    if needs.needs_cwd() {
        let basename = git.cwd_basename()?;
        Ok(result.replace("{cwd}", &basename))
    } else {
        Ok(result)
    }
}

/// Replaces the branch placeholder when present.
fn replace_branch_placeholder(
    result: String,
    git: &impl GitContext,
    needs: &PlaceholderNeeds,
) -> Result<String, ConfigError> {
    if needs.needs_branch() {
        let branch = git
            .branch_name()?
            .ok_or_else(unresolvable_branch_placeholder)?;
        Ok(result.replace("{branch}", &branch))
    } else {
        Ok(result)
    }
}

/// Builds the domain error for an unavailable branch name.
fn unresolvable_branch_placeholder() -> ConfigError {
    ConfigError::UnresolvablePlaceholder {
        placeholder: "branch".to_owned(),
        reason: concat!(
            "Not on a named branch ",
            "(detached HEAD or not a git repository)"
        )
        .to_owned(),
    }
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
