use crate::common::crawk_bin_only;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// bin-only crate (no [lib] target) — list command
// ============================================================================

#[test]
fn should_list_bin_only_crate() {
    assert_cmd_snapshot!(crawk_bin_only().arg("list"));
}

#[test]
fn should_list_bin_only_with_source() {
    assert_cmd_snapshot!(crawk_bin_only().args(["list", "-s"]));
}

#[test]
fn should_list_bin_only_table_format() {
    assert_cmd_snapshot!(crawk_bin_only().args(["list", "--format", "table"]));
}

#[test]
fn should_list_bin_only_with_depth() {
    assert_cmd_snapshot!(crawk_bin_only().args(["list", "-d", "1"]));
}
