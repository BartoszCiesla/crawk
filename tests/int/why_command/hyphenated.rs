use crate::common::crawk_hyphenated;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// hyphenated package (`hyphenated-pkg`, library crate `hyphenated_pkg`) — why command
// ============================================================================

#[test]
fn should_why_hyphenated_bin_to_lib_module() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "bin@hyphenated-pkg", "shapes"]));
}

// A crate-root re-export resolves to the synthetic `lib` target.
#[test]
fn should_why_hyphenated_bin_to_lib_root() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "bin@hyphenated-pkg", "lib"]));
}

#[test]
fn should_why_hyphenated_test_module_to_lib_module() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "support", "shapes", "-t"]));
}

// The integration test's root names `render` only in qualified paths — a call
// and a macro argument — with no `use` item behind them.
#[test]
fn should_why_hyphenated_test_root_to_lib_module_by_qualified_paths() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "test@smoke", "render", "-t"]));
}

// The integration test's root, `test@<cargo name>`, is a valid SOURCE.
#[test]
fn should_why_hyphenated_test_root_to_test_module() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "test@smoke", "support", "-t"]));
}

// So is a module path led by that root's name.
#[test]
fn should_why_hyphenated_test_module_led_by_test_root() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["why", "test@smoke::support", "shapes", "-t"]));
}
