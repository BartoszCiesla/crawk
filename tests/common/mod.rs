use insta_cmd::get_cargo_bin;
use std::process::Command;

/// Returns insta filters that strip anyhow's "Stack backtrace:" block from snapshot output.
/// Apply to any test that captures stderr from a `exit_code: 1` error path so the test
/// is not sensitive to the `RUST_BACKTRACE` environment variable.
pub(crate) fn backtrace_filters() -> Vec<(&'static str, &'static str)> {
    vec![(r"(?s)\n\nStack backtrace:.*", "")]
}

pub(crate) fn crawk() -> Command {
    Command::new(get_cargo_bin("crawk"))
}

pub(crate) fn crawk_modules() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/modules"));
    cmd
}

pub(crate) fn crawk_workspace() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/workspace"));
    cmd
}
