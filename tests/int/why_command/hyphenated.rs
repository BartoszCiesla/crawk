use crate::common::crawk_hyphenated;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// hyphenated package (`hyphenated-pkg`, library crate `hyphenated_pkg`) — why command
// ============================================================================

#[test]
fn should_why_hyphenated_bin_to_lib_module() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "main", "shapes"]));
}

// A crate-root re-export resolves to the synthetic `lib` target.
#[test]
fn should_why_hyphenated_bin_to_lib_root() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "main", "lib"]));
}

#[test]
fn should_why_hyphenated_test_module_to_lib_module() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "support", "shapes", "-t"]));
}
