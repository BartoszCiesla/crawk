use crate::common::crawk_check;
use insta_cmd::assert_cmd_snapshot;

// Reversed "app" order turns every real edge into an upward dependency.
#[test]
fn should_report_upward_violations() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_layers_violated.toml")
    );
}

// `--show-apis` annotates each offending edge with the symbols that create it.
#[test]
fn should_annotate_violations_with_apis() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_layers_violated.toml")
            .arg("--show-apis")
    );
}
