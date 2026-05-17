use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Flag combinations
// ============================================================================

#[test]
fn should_why_recursive() {
    assert_cmd_snapshot!(crawk().args(["why", "-r", "parser", "reference"]));
}

#[test]
fn should_why_recursive_fixture() {
    assert_cmd_snapshot!(crawk_modules().args(["why", "-r", "engine", "config"]));
}

#[test]
fn should_why_format_grouped() {
    assert_cmd_snapshot!(crawk().args(["why", "-r", "parser", "reference", "-f", "grouped"]));
}

#[test]
fn should_why_format_plain_explicit() {
    assert_cmd_snapshot!(crawk().args(["why", "analyzer", "reference", "-f", "plain"]));
}

#[test]
fn should_why_include_tests() {
    assert_cmd_snapshot!(crawk().args(["why", "-t", "analyzer", "reference"]));
}
