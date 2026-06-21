use crate::common::crawk;
use insta_cmd::assert_cmd_snapshot;

#[test]
fn should_show_check_help() {
    assert_cmd_snapshot!(crawk().arg("check").arg("--help"));
}
