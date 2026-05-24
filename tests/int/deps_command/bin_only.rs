use crate::common::crawk_bin_only;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// bin-only crate (no [lib] target) — deps command
// ============================================================================

#[test]
fn should_deps_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().arg("deps"));
}

#[test]
fn should_deps_bin_only_dot_format() {
    assert_cmd_snapshot!(crawk_bin_only().args(["deps", "--format", "dot"]));
}

#[test]
fn should_deps_bin_only_grouped_format() {
    assert_cmd_snapshot!(crawk_bin_only().args(["deps", "--format", "grouped"]));
}

#[test]
fn should_deps_bin_only_with_depth() {
    assert_cmd_snapshot!(crawk_bin_only().args(["deps", "-d", "1"]));
}
