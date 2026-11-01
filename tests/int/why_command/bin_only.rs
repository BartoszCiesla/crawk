use crate::common::crawk_bin_only;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// bin-only crate (no [lib] target) — why command
// ============================================================================

#[test]
fn should_why_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["why", "bin@bin_only", "config"]));
}

#[test]
fn should_why_bin_only_reverse_no_dependency() {
    assert_cmd_snapshot!(crawk_bin_only().args(["why", "config", "bin@bin_only"]));
}

#[test]
fn should_why_bin_only_transitive() {
    assert_cmd_snapshot!(crawk_bin_only().args(["why", "bin@bin_only", "utils"]));
}
