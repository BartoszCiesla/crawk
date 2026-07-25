use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Basic output — fixture crate and own crate
// ============================================================================

#[test]
fn should_deps_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("deps"));
}

#[test]
fn should_deps_for_fixture_crate() {
    assert_cmd_snapshot!(crawk_modules().arg("deps"));
}

#[test]
fn should_deps_with_include_tests_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("deps").arg("-t"));
}

#[test]
fn should_deps_with_include_tests_for_fixture_crate() {
    assert_cmd_snapshot!(crawk_modules().arg("deps").arg("-t"));
}

// ============================================================================
// Crate-root re-export edge (regression, not a snapshot: must fail loudly
// against the pre-fix resolver instead of silently recording the dropped
// edge as a snapshot)
// ============================================================================

#[test]
fn should_deps_include_crate_root_reexport_edge() {
    let output = crawk_modules()
        .arg("deps")
        .output()
        .expect("failed to run crawk");

    assert!(output.status.success(), "crawk exited with failure");

    let stdout = String::from_utf8(output.stdout).expect("stdout was not valid UTF-8");

    assert!(
        stdout
            .lines()
            .any(|line| line.trim() == "crate_root_reexport -> lib"),
        "expected an edge from crate_root_reexport to lib \
         (crate::Standalone is a crate-root re-export and must not be silently dropped):\n{stdout}"
    );
}
