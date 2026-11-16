//! End-to-end tests for configuration read from the process environment.
//!
//! `Settings::from_env` reads the live process environment on purpose, so these
//! cases run the generated `dear-diary` binary with a child environment, which
//! `AGENTS.md` permits because nothing in this test process changes. Each case
//! stops at the configuration step, before the binary builds an embedding
//! provider or opens a connection, so none needs a running Qdrant server.

use std::process::{Command, Output};

use rstest::rstest;

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

/// Runs the binary with no settings variables except `vars`, in `directory`.
#[expect(
    clippy::expect_used,
    reason = "Test fixture setup may panic on failure"
)]
fn run_with(vars: &[(&str, &std::ffi::OsStr)], directory: &std::path::Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dear-diary"));
    command.current_dir(directory);
    for key in SETTINGS_ENV_KEYS {
        command.env_remove(key);
    }
    for (key, value) in vars {
        command.env(key, value);
    }
    command.output().expect("run the dear-diary binary")
}

/// The binary's standard error as text.
fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[rstest]
fn a_missing_connection_is_reported_as_a_configuration_error() {
    let directory = std::env::temp_dir();

    let output = run_with(&[], &directory);

    assert!(!output.status.success());
    assert!(
        stderr_of(&output)
            .contains("Configuration error: Must specify either QDRANT_URL or QDRANT_LOCAL_PATH"),
        "unexpected stderr: {}",
        stderr_of(&output)
    );
}

#[rstest]
fn both_connection_modes_are_reported_as_a_configuration_error() {
    let directory = std::env::temp_dir();

    let output = run_with(
        &[
            ("QDRANT_URL", "http://localhost:6334".as_ref()),
            ("QDRANT_LOCAL_PATH", "/tmp/qdrant".as_ref()),
        ],
        &directory,
    );

    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("Configuration error: Cannot specify both"),
        "unexpected stderr: {}",
        stderr_of(&output)
    );
}

#[rstest]
fn an_unresolvable_placeholder_is_reported_from_the_environment() {
    let directory = std::env::temp_dir();

    let output = run_with(
        &[
            ("QDRANT_URL", "http://localhost:6334".as_ref()),
            ("COLLECTION_NAME", "{owner}-{repo}".as_ref()),
        ],
        &directory,
    );

    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains(
            "Configuration error: Git command failed during COLLECTION_NAME interpolation"
        ),
        "unexpected stderr: {}",
        stderr_of(&output)
    );
}

/// A value that is not valid Unicode reads as absent, so the missing connection
/// is what the binary reports.
#[cfg(unix)]
#[rstest]
fn a_non_unicode_connection_url_reads_as_absent() {
    use std::os::unix::ffi::OsStrExt as _;

    let directory = std::env::temp_dir();
    let invalid = std::ffi::OsStr::from_bytes(&[0xff, 0xfe]);

    let output = run_with(&[("QDRANT_URL", invalid)], &directory);

    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("Configuration error: Must specify either QDRANT_URL"),
        "unexpected stderr: {}",
        stderr_of(&output)
    );
}
