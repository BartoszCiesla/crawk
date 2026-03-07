use crate::common::crawk;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// Overview and help messages tests
// ============================================================================

#[test]
fn should_use_command_overview_match() {
    assert_cmd_snapshot!(crawk().arg("use"));
}

#[test_case("-h"; "short help")]
#[test_case("--help"; "long help")]
fn should_use_command_short_help_match(help_flag: &str) {
    let flag = help_flag.trim_start_matches('-');
    let snapshot_name = format!("flag_{flag}");

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(help_flag));
}
