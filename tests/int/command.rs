use crate::common::crawk;
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

#[test]
fn should_command_overview_match() {
    assert_cmd_snapshot!(crawk());
}

#[test_case("-h"; "short help")]
#[test_case("--help"; "long help")]
fn should_command_short_help_match(help_flag: &str) {
    let flag = help_flag.trim_start_matches('-');
    let snapshot_name = format!("flag_{flag}");

    // Filter out non-deterministic part with build timestamp
    with_settings!({
    filters => vec![
        (r"Built on \d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z for [\w-]+ \([\w.-]+\) by \w+",
         "Built on [TIMESTAMP] for [TARGET] ([RUSTC]) by [USER]"),
    ],
    }, {
        assert_cmd_snapshot!(snapshot_name, crawk().arg(help_flag));
    });
}
