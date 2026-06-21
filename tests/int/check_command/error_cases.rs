use crate::common::crawk_check;
use insta_cmd::assert_cmd_snapshot;

// A rule naming a non-existent module → UnknownRuleModule (exit 2).
#[test]
fn should_error_on_unknown_module() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_bad_module.toml")
    );
}

// A module matched by two layer groups → ambiguous config (exit 2).
#[test]
fn should_error_on_overlapping_groups() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_overlapping_groups.toml")
    );
}
