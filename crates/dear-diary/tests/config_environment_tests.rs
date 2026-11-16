//! End-to-end tests for configuration read from the process environment.
//!
//! `Settings::from_env` reads the live process environment on purpose, so these
//! cases run the generated `dear-diary` binary through `assert_cmd` with a child
//! environment, which `AGENTS.md` permits because nothing in this test process
//! changes. Each case stops at the configuration step, before the binary builds
//! an embedding provider or opens a connection, so none needs a running Qdrant
//! server.

use std::ffi::OsStr;
use std::path::Path;

use assert_cmd::Command;
use rstest::rstest;
use tempfile::TempDir;

/// Every variable the configuration reads, removed from the child so the
/// parent's environment cannot change a case. The rest of the parent's
/// environment is inherited, which keeps `LLVM_PROFILE_FILE` for coverage runs.
const SETTINGS_ENV_KEYS: [&str; 10] = [
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

/// Variables that make Git pick a repository other than the one found from the
/// working directory, removed from the child so interpolation sees only the
/// directory a case names.
const GIT_OVERRIDE_KEYS: [&str; 6] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

/// A working directory that is not inside any Git repository.
///
/// Git is stopped from searching above it with `GIT_CEILING_DIRECTORIES` (see
/// [`run_with`]), so a repository that happens to enclose the system temporary
/// directory cannot change a case.
struct BareDirectory {
    directory: TempDir,
}

impl BareDirectory {
    #[expect(
        clippy::expect_used,
        reason = "Test fixture setup may panic on failure"
    )]
    fn new() -> Self {
        Self {
            directory: TempDir::new().expect("create a working directory"),
        }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }
}

/// Runs the binary with no settings variables except `vars`, in `directory`,
/// and returns its standard error as text after checking it failed.
#[expect(
    clippy::expect_used,
    reason = "Test fixture setup may panic on failure"
)]
fn failing_stderr(vars: &[(&str, &OsStr)], directory: &BareDirectory) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dear-diary"));
    command.current_dir(directory.path());
    // Stop Git discovering a repository above the bare directory.
    command.env("GIT_CEILING_DIRECTORIES", directory.path());
    for key in SETTINGS_ENV_KEYS.iter().chain(&GIT_OVERRIDE_KEYS) {
        command.env_remove(key);
    }
    for (key, value) in vars {
        command.env(key, value);
    }
    let assert = command.assert().failure();
    String::from_utf8(assert.get_output().stderr.clone()).expect("stderr is UTF-8")
}

#[rstest]
fn a_missing_connection_is_reported_as_a_configuration_error() {
    let stderr = failing_stderr(&[], &BareDirectory::new());

    assert!(
        stderr.contains("Configuration error: Must specify either QDRANT_URL or QDRANT_LOCAL_PATH"),
        "unexpected stderr: {stderr}"
    );
}

#[rstest]
fn both_connection_modes_are_reported_as_a_configuration_error() {
    let stderr = failing_stderr(
        &[
            ("QDRANT_URL", "http://localhost:6334".as_ref()),
            ("QDRANT_LOCAL_PATH", "/tmp/qdrant".as_ref()),
        ],
        &BareDirectory::new(),
    );

    assert!(
        stderr.contains("Configuration error: Cannot specify both"),
        "unexpected stderr: {stderr}"
    );
}

/// The working directory is a bare directory and Git is held to it, so the
/// interpolation can only fail with Git's own "not a git repository" report.
#[rstest]
fn an_unresolvable_placeholder_is_reported_from_the_environment() {
    let stderr = failing_stderr(
        &[
            ("QDRANT_URL", "http://localhost:6334".as_ref()),
            ("COLLECTION_NAME", "{owner}-{repo}".as_ref()),
        ],
        &BareDirectory::new(),
    );

    assert!(
        stderr.contains(
            "Configuration error: Git command failed during COLLECTION_NAME interpolation"
        ),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains("not a git repository"),
        "unexpected stderr: {stderr}"
    );
}

/// A value that is not valid Unicode reads as absent, so the missing connection
/// is what the binary reports.
#[cfg(unix)]
#[rstest]
fn a_non_unicode_connection_url_reads_as_absent() {
    use std::os::unix::ffi::OsStrExt as _;

    let invalid = OsStr::from_bytes(&[0xff, 0xfe]);

    let stderr = failing_stderr(&[("QDRANT_URL", invalid)], &BareDirectory::new());

    assert!(
        stderr.contains("Configuration error: Must specify either QDRANT_URL"),
        "unexpected stderr: {stderr}"
    );
}
