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
