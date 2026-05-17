use crate::common::crawk;
use insta_cmd::assert_cmd_snapshot;

#[test]
fn should_show_why_help() {
    assert_cmd_snapshot!(crawk().args(["why", "--help"]));
}

#[test]
fn should_show_why_short_help() {
    assert_cmd_snapshot!(crawk().args(["why", "-h"]));
}
