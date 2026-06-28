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

// A module shared by two overlapping groups is checked independently in each;
// when both orderings are satisfied the run is clean (exit 0).
#[test]
fn should_allow_overlapping_groups_when_clean() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_overlapping_clean.toml")
    );
}

// An edge that points upward in two overlapping groups yields one violation per
// group (exit 1) — per-group reporting names each offending group.
#[test]
fn should_report_violation_per_overlapping_group() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_overlapping_groups.toml")
    );
}
