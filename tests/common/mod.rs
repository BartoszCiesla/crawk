//! Shared helpers for the integration tests: crawk command builders and
//! insta filters.

use insta_cmd::get_cargo_bin;
use std::path::Path;
use std::process::Command;

/// Returns insta filters that strip anyhow's "Stack backtrace:" block from snapshot output.
/// Apply to any test that captures stderr from a `exit_code: 1` error path so the test
/// is not sensitive to the `RUST_BACKTRACE` environment variable.
pub(crate) fn backtrace_filters() -> Vec<(&'static str, &'static str)> {
    vec![(r"(?s)\n\nStack backtrace:.*", "")]
}

/// Returns insta filters that collapse a canonicalized, machine-specific crate
/// root (e.g. `/home/user/crawk/fixtures/check`) down to `[ROOT]/fixtures/...`.
/// Apply to any test whose output embeds an absolute crate path.
pub(crate) fn crate_root_filters() -> Vec<(&'static str, &'static str)> {
    // Lazily consume the absolute prefix up to `/fixtures/`. Exclude only `)`
    // (the path appears inside `(...)`); allowing spaces keeps the filter working
    // when the checkout path itself contains a space.
    vec![(r"/[^)]*?/fixtures/", "[ROOT]/fixtures/")]
}

pub(crate) fn crawk() -> Command {
    Command::new(get_cargo_bin("crawk"))
}

pub(crate) fn crawk_modules() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p").arg("fixtures/modules");
    cmd
}

pub(crate) fn crawk_cycles() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p").arg("fixtures/cycles");
    cmd
}

pub(crate) fn crawk_check() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p").arg("fixtures/check").arg("check");
    cmd
}

pub(crate) fn crawk_workspace() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p").arg("fixtures/workspace");
    cmd
}

pub(crate) fn crawk_bin_only() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p").arg("fixtures/bin_only");
    cmd
}

pub(crate) fn crawk_paths() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p").arg("fixtures/paths");
    cmd
}

pub(crate) fn crawk_hyphenated() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p").arg("fixtures/hyphenated");
    cmd
}

/// Runs crawk on the crate at `root` and returns its stdout, asserting it
/// exited with `code`.
///
/// For crates written into a tempdir: their path would leak into the header of
/// an `insta` snapshot, so such tests assert on stdout directly.
///
/// Not itself a `#[test]` fn, so clippy's `allow-expect-in-tests` doesn't
/// cover it; it's test-support scaffolding, so `expect` is fine here.
#[allow(clippy::expect_used)]
pub(crate) fn crawk_stdout(root: &Path, code: i32, args: &[&str]) -> String {
    let output = crawk()
        .arg("-p")
        .arg(root)
        .args(args)
        .output()
        .expect("run crawk");
    assert_eq!(
        output.status.code(),
        Some(code),
        "crawk {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is utf-8")
}
