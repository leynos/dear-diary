//! Git remote URL parsing for `COLLECTION_NAME` interpolation.
//!
//! Extracts owner and repository names from git remote URLs in HTTPS,
//! SSH, and SCP-style formats.

use bstr::ByteSlice;

use crate::error::ConfigError;

/// Parsed owner and repository name from a git remote URL.
///
/// `owner` is `None` when the remote path contains only a single
/// segment (e.g. `https://git.example.com/myrepo.git`), which is
/// valid for self-hosted setups that expose repositories at the host
/// root.
#[derive(Debug)]
pub(crate) struct RemoteInfo {
    pub owner: Option<String>,
    pub repo: String,
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
/// Single-segment paths (e.g. `https://host/repo.git`) set `owner` to
/// `None` and place the sole segment in `repo`.
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed or contains no
/// recognisable path segments.
pub(crate) fn parse_remote_url(raw_url: &str) -> Result<RemoteInfo, ConfigError> {
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

    // Walk segments once, tracking first and last non-empty values.
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

    match segment_count {
        0 => Err(ConfigError::InterpolationContextError(format!(
            concat!(
                "Cannot extract repository from remote URL ",
                "'{0}': path contains no segments"
            ),
            raw_url
        ))),
        1 => {
            // Single segment — repo only, no owner.
            // SAFETY (logic): segment_count == 1 guarantees
            // last_segment is Some (set during the loop above).
            let repo = last_segment.ok_or_else(|| {
                ConfigError::InterpolationContextError(format!(
                    "Cannot extract repository from remote URL \
                     '{raw_url}'"
                ))
            })?;
            Ok(RemoteInfo {
                owner: None,
                repo: repo.to_owned(),
            })
        }
        _ => {
            // Two or more segments — first is owner, last is repo.
            // SAFETY (logic): segment_count >= 2 guarantees both Some.
            let owner_raw = first_segment.ok_or_else(|| {
                ConfigError::InterpolationContextError(format!(
                    "Cannot extract owner/repo from remote URL \
                     '{raw_url}'"
                ))
            })?;
            let repo = last_segment.ok_or_else(|| {
                ConfigError::InterpolationContextError(format!(
                    "Cannot extract owner/repo from remote URL \
                     '{raw_url}'"
                ))
            })?;

            // Strip Source Hut tilde prefix from owner.
            let owner = owner_raw.strip_prefix('~').unwrap_or(owner_raw);

            Ok(RemoteInfo {
                owner: Some(owner.to_owned()),
                repo: repo.to_owned(),
            })
        }
    }
}
