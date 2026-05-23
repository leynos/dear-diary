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
    let segments = RemotePathSegments::from_path(clean_path);

    remote_info_from_segments(raw_url, &segments)
}

/// First, last, and count metadata for a remote path.
struct RemotePathSegments<'a> {
    first: Option<&'a str>,
    last: Option<&'a str>,
    count: usize,
}

impl<'a> RemotePathSegments<'a> {
    /// Builds segment metadata without allocating a segment vector.
    fn from_path(clean_path: &'a str) -> Self {
        let mut first = None;
        let mut last = None;
        let mut count = 0;

        for segment in clean_path.split('/').filter(|s| !s.is_empty()) {
            first.get_or_insert(segment);
            last = Some(segment);
            count += 1;
        }

        Self { first, last, count }
    }

    /// Returns true when the path contains no repository segment.
    const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns true when the path contains a repository without an owner.
    const fn is_single_segment(&self) -> bool {
        self.count == 1
    }
}

/// Builds remote metadata from parsed path segments.
fn remote_info_from_segments(
    raw_url: &str,
    segments: &RemotePathSegments<'_>,
) -> Result<RemoteInfo, ConfigError> {
    if segments.is_empty() {
        return Err(empty_remote_path_error(raw_url));
    }

    if segments.is_single_segment() {
        return Ok(RemoteInfo {
            owner: None,
            repo: last_remote_segment(raw_url, segments)?.to_owned(),
        });
    }

    let owner_raw = first_remote_segment(raw_url, segments)?;
    let owner = owner_raw.strip_prefix('~').unwrap_or(owner_raw);

    Ok(RemoteInfo {
        owner: Some(owner.to_owned()),
        repo: last_remote_segment(raw_url, segments)?.to_owned(),
    })
}

/// Builds the domain error for remote URLs without path segments.
fn empty_remote_path_error(raw_url: &str) -> ConfigError {
    ConfigError::InterpolationContextError(format!(
        concat!(
            "Cannot extract repository from remote URL ",
            "'{0}': path contains no segments"
        ),
        raw_url
    ))
}

/// Returns the first remote path segment.
fn first_remote_segment<'a>(
    raw_url: &str,
    segments: &RemotePathSegments<'a>,
) -> Result<&'a str, ConfigError> {
    segments.first.ok_or_else(|| {
        ConfigError::InterpolationContextError(format!(
            "Cannot extract owner/repo from remote URL \
             '{raw_url}'"
        ))
    })
}

/// Returns the last remote path segment.
fn last_remote_segment<'a>(
    raw_url: &str,
    segments: &RemotePathSegments<'a>,
) -> Result<&'a str, ConfigError> {
    segments.last.ok_or_else(|| {
        ConfigError::InterpolationContextError(format!(
            "Cannot extract repository from remote URL \
             '{raw_url}'"
        ))
    })
}
