use crate::common::crawk_check;
use insta_cmd::assert_cmd_snapshot;

// `deny cli -> web::*` catches the cross-group edge cli -> web::repo that layers
// leaves unconstrained; the layers group is satisfied, so the report is a single
// DENY row (exit 1).
#[test]
fn should_report_denied_edge() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_deny_violated.toml")
    );
}

// `--show-apis` annotates the offending edge with the symbols that create it.
#[test]
fn should_annotate_denied_edge_with_apis() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_deny_violated.toml")
            .arg("--show-apis")
    );
}

// Deny rules that match no edge are satisfied: no output, exit 0.
#[test]
fn should_pass_when_no_edge_matches() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_deny_clean.toml")
    );
}
