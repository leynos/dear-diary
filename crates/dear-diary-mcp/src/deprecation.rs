//! Deprecation visibility logic for search results.

/// Number of seconds in 7 days for deprecation threshold.
pub(crate) const SEVEN_DAYS_SECS: i64 = 7 * 24 * 3600;

/// Result of checking deprecation visibility for a search result.
pub(crate) struct DeprecationVisibility {
    /// Whether to include this result in the output.
    pub include: bool,
    /// Whether this result is marked as deprecated.
    pub is_deprecated: bool,
}

/// Determines visibility of a result based on its deprecation state.
///
/// Returns whether to include the result and whether to show it as deprecated.
/// - Active entries (no deprecation): always included, not marked deprecated
/// - Recently deprecated (< 7 days): included, marked deprecated
/// - Old deprecated (>= 7 days): hidden unless `include_deprecated` is true
pub(crate) const fn visibility(
    now: i64,
    deprecated_at: Option<i64>,
    include_deprecated: bool,
) -> DeprecationVisibility {
    match deprecated_at {
        None => DeprecationVisibility {
            include: true,
            is_deprecated: false,
        },
        Some(ts) if now - ts < SEVEN_DAYS_SECS => DeprecationVisibility {
            include: true,
            is_deprecated: true,
        },
        Some(_) if include_deprecated => DeprecationVisibility {
            include: true,
            is_deprecated: true,
        },
        Some(_) => DeprecationVisibility {
            include: false,
            is_deprecated: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Test that active entries (no deprecation) are always visible.
    #[rstest]
    fn test_active_entry_visible() {
        let result = visibility(1000, None, false);
        assert!(result.include);
        assert!(!result.is_deprecated);
    }

    /// Test that recently deprecated entries (< 7 days) are visible with marker.
    #[rstest]
    fn test_recent_deprecation_visible() {
        let now = 1000;
        let deprecated_at = now - (SEVEN_DAYS_SECS - 100); // Less than 7 days ago
        let result = visibility(now, Some(deprecated_at), false);
        assert!(result.include);
        assert!(result.is_deprecated);
    }

    /// Test that old deprecated entries (>= 7 days) are hidden by default.
    #[rstest]
    fn test_old_deprecation_hidden() {
        let now = 1_000_000;
        let deprecated_at = now - SEVEN_DAYS_SECS - 100; // More than 7 days ago
        let result = visibility(now, Some(deprecated_at), false);
        assert!(!result.include);
    }

    /// Test that old deprecated entries are visible when `include_deprecated` is true.
    #[rstest]
    fn test_old_deprecation_visible_when_included() {
        let now = 1_000_000;
        let deprecated_at = now - SEVEN_DAYS_SECS - 100; // More than 7 days ago
        let result = visibility(now, Some(deprecated_at), true);
        assert!(result.include);
        assert!(result.is_deprecated);
    }
}
