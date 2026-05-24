use crate::common::crawk_bin_only;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// bin-only crate (no [lib] target) — use command
// ============================================================================

#[test]
fn should_use_main_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main"]));
}

#[test]
fn should_use_main_recursive_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main", "-r"]));
}

#[test]
fn should_use_nested_module_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main::utils::helper"]));
}

#[test]
fn should_use_main_with_expand_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main", "-e"]));
}

#[test]
fn should_use_main_grouped_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main", "-e", "--format", "grouped"]));
}
