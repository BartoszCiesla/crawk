use crate::common::crawk;
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ==========================================================================
// Version flag tests
// ==========================================================================
#[test_case("-V"; "short version")]
#[test_case("--version"; "long version")]
fn should_show_version(flag: &str) {
    let flag_name = flag.trim_start_matches('-');
    let snapshot_name = format!("version_flag_{flag_name}");

    with_settings!({
        filters => vec![
            (r"crawk \d+\.\d+\.\d+.*", "crawk [VERSION]"),
        ],
    }, {
        assert_cmd_snapshot!(snapshot_name, crawk().arg(flag));
    });
}
