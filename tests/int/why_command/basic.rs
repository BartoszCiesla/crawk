use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Basic output — fixture crate and own crate
// ============================================================================

#[test]
fn should_why_for_own_crate() {
    assert_cmd_snapshot!(crawk().args(["why", "analyzer", "reference"]));
}

#[test]
fn should_why_for_fixture_crate() {
    assert_cmd_snapshot!(crawk_modules().args(["why", "engine", "config"]));
}

#[test]
fn should_why_no_dependency() {
    assert_cmd_snapshot!(crawk_modules().args(["why", "config", "engine"]));
}

#[test]
fn should_why_no_dependency_own_crate() {
    assert_cmd_snapshot!(crawk().args(["why", "constants", "analyzer"]));
}
